//! Public resolver qualification against local DNS/DoT/DoH servers.

use std::{
    io,
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, Mutex},
    time::Duration,
};

use n0_dns_resolver::{DnsProtocol, DnsResolver, Error, Nameserver, NativeSocketInitializer};
use simple_dns::{
    CLASS, Packet, PacketFlag, ResourceRecord,
    rdata::{A, RData},
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, UdpSocket},
};

fn response(query: &[u8]) -> Vec<u8> {
    let query = Packet::parse(query).unwrap();
    let mut response = Packet::new_reply(query.id());
    response.set_flags(PacketFlag::RECURSION_AVAILABLE);
    response.questions = query.questions.clone();
    response.answers.push(ResourceRecord::new(
        query.questions[0].qname.clone(),
        CLASS::IN,
        300,
        RData::A(A {
            address: u32::from(Ipv4Addr::new(192, 0, 2, 10)),
        }),
    ));
    response.build_bytes_vec().unwrap()
}

async fn serve_stream(mut stream: impl AsyncRead + AsyncWrite + Unpin, https: bool) {
    loop {
        let query = if https {
            let mut headers = Vec::new();
            while !headers.ends_with(b"\r\n\r\n") {
                let Ok(byte) = stream.read_u8().await else {
                    return;
                };
                headers.push(byte);
                assert!(headers.len() < 16384);
            }
            let headers = String::from_utf8(headers).unwrap();
            assert!(headers.starts_with("POST /dns-query HTTP/1.1\r\n"));
            let len: usize = headers
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length: ")?
                        .parse()
                        .ok()
                })
                .unwrap();
            let mut query = vec![0; len];
            stream.read_exact(&mut query).await.unwrap();
            query
        } else {
            let Ok(len) = stream.read_u16().await else {
                return;
            };
            let mut query = vec![0; usize::from(len)];
            stream.read_exact(&mut query).await.unwrap();
            query
        };
        let response = response(&query);
        if https {
            let headers = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/dns-message\r\nContent-Length: {}\r\n\r\n",
                response.len()
            );
            stream.write_all(headers.as_bytes()).await.unwrap();
        } else {
            stream
                .write_u16(response.len().try_into().unwrap())
                .await
                .unwrap();
        }
        stream.write_all(&response).await.unwrap();
        stream.flush().await.unwrap();
    }
}

struct Server {
    addr: SocketAddr,
    tls: rustls::ClientConfig,
    task: tokio::task::JoinHandle<()>,
}

async fn server(protocol: DnsProtocol) -> Server {
    let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut roots = rustls::RootCertStore::empty();
    roots.add(certificate.cert.der().clone()).unwrap();
    let tls = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![certificate.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der())
                .into(),
        )
        .unwrap();
    if protocol == DnsProtocol::Udp {
        let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr = socket.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let mut buf = vec![0; 4096];
            loop {
                let (len, peer) = socket.recv_from(&mut buf).await.unwrap();
                socket.send_to(&response(&buf[..len]), peer).await.unwrap();
            }
        });
        return Server { addr, tls, task };
    }
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(config));
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                if protocol == DnsProtocol::Tcp {
                    serve_stream(stream, false).await;
                } else {
                    serve_stream(
                        acceptor.accept(stream).await.unwrap(),
                        protocol == DnsProtocol::Https,
                    )
                    .await;
                }
            });
        }
    });
    Server { addr, tls, task }
}

fn nameserver(server: &Server, protocol: DnsProtocol) -> Nameserver {
    Nameserver::with_server_name(server.addr, protocol, "localhost")
}

#[tokio::test]
async fn policy_covers_all_fresh_transports_and_survives_reset() {
    for protocol in [
        DnsProtocol::Udp,
        DnsProtocol::Tcp,
        DnsProtocol::Tls,
        DnsProtocol::Https,
    ] {
        let server = server(protocol).await;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observed = calls.clone();
        let initializer = NativeSocketInitializer::new(move |socket, actual| {
            let socket = socket2::SockRef::from(&socket);
            match socket.local_addr() {
                Ok(addr) => assert_eq!(
                    addr.as_socket().unwrap().port(),
                    0,
                    "must precede bind/connect"
                ),
                // Winsock getsockname reports WSAEINVAL for an unbound socket.
                #[cfg(windows)]
                Err(err) if err.raw_os_error() == Some(10022) => {}
                Err(err) => return Err(err),
            }
            assert!(socket.peer_addr().is_err());
            observed.lock().unwrap().push(actual);
            Ok(())
        });
        let resolver = DnsResolver::builder()
            .nameserver(nameserver(&server, protocol))
            .tls_client_config(server.tls.clone())
            .native_socket_initializer(initializer)
            .build();
        let expected = vec![Ipv4Addr::new(192, 0, 2, 10)];
        assert_eq!(
            resolver.lookup_ipv4("first.example.").await.unwrap(),
            expected
        );
        assert_eq!(*calls.lock().unwrap(), vec![protocol]);
        assert_eq!(
            resolver.lookup_ipv4("first.example.").await.unwrap(),
            expected
        );
        assert_eq!(
            calls.lock().unwrap().len(),
            1,
            "cached answer creates no socket"
        );
        assert_eq!(
            resolver.lookup_ipv4("second.example.").await.unwrap(),
            expected
        );
        let before_reset = calls.lock().unwrap().len();
        assert_eq!(
            before_reset,
            if protocol == DnsProtocol::Udp { 2 } else { 1 },
            "existing pooling is preserved"
        );
        let reset = resolver.reset();
        assert_eq!(reset.lookup_ipv4("third.example.").await.unwrap(), expected);
        assert_eq!(calls.lock().unwrap().len(), before_reset + 1);
        server.task.abort();
    }
}

#[tokio::test]
async fn refusal_is_terminal_before_transport_or_nameserver_fallback() {
    for protocol in [
        DnsProtocol::Udp,
        DnsProtocol::Tcp,
        DnsProtocol::Tls,
        DnsProtocol::Https,
    ] {
        let server = server(protocol).await;
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observed = calls.clone();
        let initializer = NativeSocketInitializer::new(move |_, actual| {
            observed.lock().unwrap().push(actual);
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "helper generation refused",
            ))
        });
        let resolver = DnsResolver::builder()
            .nameserver(nameserver(&server, protocol))
            .fallback_nameservers([nameserver(&server, DnsProtocol::Tcp)])
            .tls_client_config(server.tls.clone())
            .native_socket_initializer(initializer)
            .build();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            resolver.lookup_ipv4("refused.example."),
        )
        .await
        .unwrap();
        assert!(
            matches!(result, Err(Error::SocketPolicy { .. })),
            "{protocol:?}: {result:?}"
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![protocol],
            "no UDP retransmit, TCP fallback or next nameserver after refusal"
        );
        server.task.abort();
    }
}
