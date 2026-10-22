use std::{
    net::Ipv4Addr,
    os::fd::AsFd,
    sync::atomic::{AtomicUsize, Ordering},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;

#[tokio::test]
async fn tcp_policy_reaches_each_actual_attempt_before_connect() {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let resolver = crate::test_utils::static_resolver(
        vec![Ipv4Addr::new(127, 0, 0, 2), Ipv4Addr::LOCALHOST],
        vec![],
    );
    let attempts = Arc::new(AtomicUsize::new(0));
    let counted = attempts.clone();
    let initialize = NativeTcpSocketInitializer::new(move |socket| {
        assert_eq!(
            socket.peer_addr().port(),
            port,
            "initializer must receive the bound relay's port"
        );
        assert!(
            matches!(socket.peer_addr().ip(), std::net::IpAddr::V4(ip) if ip == Ipv4Addr::new(127,0,0,2) || ip == Ipv4Addr::LOCALHOST)
        );
        // This duplicate is a temporary socket-option mechanism, not a sender.
        // Inspect the real kernel descriptor before the dialer can connect it.
        let duplicate = std::net::TcpStream::from(socket.as_fd().try_clone_to_owned()?);
        assert!(
            duplicate.peer_addr().is_err(),
            "initializer must precede connect"
        );
        duplicate.set_ttl(31)?;
        counted.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let url = format!("http://relay.test:{port}").parse().unwrap();
    let mut stream = dial_happy_eyeballs(&resolver, &url, false, Some(&initialize))
        .await
        .unwrap();
    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "every fresh attempt must retain constructor policy"
    );
    assert_eq!(
        stream.ttl().unwrap(),
        31,
        "configured kernel option must belong to the connected descriptor"
    );
    let (mut accepted, _) = listener.accept().await.unwrap();
    stream.write_all(b"policy-bytes").await.unwrap();
    let mut bytes = [0; 12];
    accepted.read_exact(&mut bytes).await.unwrap();
    assert_eq!(&bytes, b"policy-bytes");
}

#[tokio::test]
async fn public_relay_and_http_proxy_refuse_policy_without_address_fallback() {
    for proxy in [false, true] {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let observed = tokio::spawn(async move {
            match tokio::time::timeout(std::time::Duration::from_millis(300), listener.accept())
                .await
            {
                Ok(Ok((mut stream, _))) => {
                    let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    true
                }
                _ => false,
            }
        });
        let attempts = Arc::new(AtomicUsize::new(0));
        let counted = attempts.clone();
        let initialize = NativeTcpSocketInitializer::new(move |_| {
            if counted.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "tcp-policy-refusal",
                ))
            } else {
                Ok(())
            }
        });
        let resolver = crate::test_utils::static_resolver(
            vec![Ipv4Addr::new(127, 0, 0, 2), Ipv4Addr::LOCALHOST],
            vec![],
        );
        let url: RelayUrl = format!("http://relay.test:{port}").parse().unwrap();
        let mut builder = ClientBuilder::new(url, SecretKey::generate(), resolver)
            .tls_client_config(
                crate::tls::CaTlsConfig::default()
                    .client_config(crate::tls::default_provider())
                    .expect("valid default TLS policy"),
            )
            .native_tcp_socket_initializer(initialize);
        if proxy {
            builder = builder.proxy_url(format!("http://proxy.test:{port}").parse().unwrap());
        }
        let error = tokio::time::timeout(std::time::Duration::from_secs(3), builder.connect())
            .await
            .unwrap()
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("tcp-policy-refusal"),
            "public constructor must propagate policy refusal: {error:#}"
        );
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            1,
            "ordinary Happy Eyeballs fallback cannot bypass a host-policy refusal"
        );
        assert!(
            !observed.await.unwrap(),
            "refused sockets must emit no TCP connection to the viable alternative"
        );
    }
}
