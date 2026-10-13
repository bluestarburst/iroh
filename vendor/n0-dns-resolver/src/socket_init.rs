//! Trusted constructor input for the resolver's existing socket factories.
use std::{fmt, io, sync::Arc};

use crate::DnsProtocol;

/// An unused socket borrowed only for synchronous host configuration.
/// Never retain its descriptor or create another connection owner.
#[derive(Debug, Clone, Copy)]
pub struct NativeSocketRef<'a>(SocketKind<'a>);

#[derive(Debug, Clone, Copy)]
enum SocketKind<'a> {
    Udp(&'a socket2::Socket),
    Tcp(&'a tokio::net::TcpSocket),
    #[cfg(transport_https)]
    Http(reqwest::NativeTcpSocketRef<'a>),
}

#[cfg(unix)]
impl std::os::fd::AsFd for NativeSocketRef<'_> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        use std::os::fd::AsFd;
        match &self.0 {
            SocketKind::Udp(socket) => AsFd::as_fd(*socket),
            SocketKind::Tcp(socket) => AsFd::as_fd(*socket),
            #[cfg(transport_https)]
            SocketKind::Http(socket) => AsFd::as_fd(socket),
        }
    }
}

#[cfg(windows)]
impl std::os::windows::io::AsSocket for NativeSocketRef<'_> {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        use std::os::windows::io::AsSocket;
        match &self.0 {
            SocketKind::Udp(socket) => AsSocket::as_socket(*socket),
            SocketKind::Tcp(socket) => AsSocket::as_socket(*socket),
            #[cfg(transport_https)]
            SocketKind::Http(socket) => AsSocket::as_socket(socket),
        }
    }
}

/// One policy retained by all native factories, pooled clients and resets.
#[derive(Clone)]
pub struct NativeSocketInitializer(Arc<SocketCallback>);

type SocketCallback =
    dyn for<'a> Fn(NativeSocketRef<'a>, DnsProtocol) -> io::Result<()> + Send + Sync;

impl fmt::Debug for NativeSocketInitializer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeSocketInitializer")
    }
}

impl NativeSocketInitializer {
    /// Called before bind/connect on each fresh socket. Must be bounded and
    /// return any refusal; the resolver treats refusal as terminal.
    pub fn new(
        initialize: impl for<'a> Fn(NativeSocketRef<'a>, DnsProtocol) -> io::Result<()>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        Self(Arc::new(initialize))
    }

    pub(crate) fn udp(&self, socket: &socket2::Socket) -> io::Result<()> {
        (self.0)(NativeSocketRef(SocketKind::Udp(socket)), DnsProtocol::Udp)
    }

    pub(crate) fn tcp(
        &self,
        socket: &tokio::net::TcpSocket,
        protocol: DnsProtocol,
    ) -> io::Result<()> {
        (self.0)(NativeSocketRef(SocketKind::Tcp(socket)), protocol)
    }

    #[cfg(transport_https)]
    pub(crate) fn http_client(&self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        let initialize = self.clone();
        builder.native_tcp_socket_initializer(move |socket| {
            (initialize.0)(
                NativeSocketRef(SocketKind::Http(socket)),
                DnsProtocol::Https,
            )
            .map_err(|err| io::Error::other(PolicyRefusal(err)))
        })
    }
}

/// Carries the policy classification through Reqwest/Hyper's error chain.
#[cfg(transport_https)]
#[derive(Debug)]
struct PolicyRefusal(io::Error);

#[cfg(transport_https)]
impl fmt::Display for PolicyRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native DNS socket protection refused: {}", self.0)
    }
}

#[cfg(transport_https)]
impl std::error::Error for PolicyRefusal {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[cfg(transport_https)]
pub(crate) fn is_policy_refusal(mut err: &(dyn std::error::Error + 'static)) -> bool {
    loop {
        if err.is::<PolicyRefusal>() {
            return true;
        }
        // io::Error's source may skip the boxed error and expose its source.
        if let Some(inner) = err.downcast_ref::<io::Error>().and_then(io::Error::get_ref)
            && is_policy_refusal(inner)
        {
            return true;
        }
        let Some(source) = err.source() else {
            return false;
        };
        err = source;
    }
}
