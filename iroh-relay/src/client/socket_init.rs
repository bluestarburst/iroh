//! Opt-in native TCP construction input; the existing dialer owns attempts.
use std::{fmt, io, sync::Arc};

/// Borrowed unused TCP socket. Configure it synchronously before connect;
/// never retain a descriptor or create another sender/lifecycle owner.
#[derive(Clone, Copy, Debug)]
pub struct NativeTcpSocketRef<'a>(NativeTcpSocketKind<'a>);
#[derive(Clone, Copy, Debug)]
enum NativeTcpSocketKind<'a> {
    Relay(&'a tokio::net::TcpSocket),
    Http(reqwest::NativeTcpSocketRef<'a>),
}

#[cfg(unix)]
impl std::os::fd::AsFd for NativeTcpSocketRef<'_> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        match &self.0 {
            NativeTcpSocketKind::Relay(socket) => std::os::fd::AsFd::as_fd(*socket),
            NativeTcpSocketKind::Http(socket) => std::os::fd::AsFd::as_fd(socket),
        }
    }
}
#[cfg(windows)]
impl std::os::windows::io::AsSocket for NativeTcpSocketRef<'_> {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        match &self.0 {
            NativeTcpSocketKind::Relay(socket) => {
                std::os::windows::io::AsSocket::as_socket(*socket)
            }
            NativeTcpSocketKind::Http(socket) => std::os::windows::io::AsSocket::as_socket(socket),
        }
    }
}

/// Constructor policy retained for every relay/proxy TCP attempt. This covers
/// no DNS, QUIC relay, HTTP gateway or custom-carrier socket factory.
#[derive(Clone)]
pub struct NativeTcpSocketInitializer(
    Arc<dyn for<'a> Fn(NativeTcpSocketRef<'a>) -> io::Result<()> + Send + Sync>,
);
impl fmt::Debug for NativeTcpSocketInitializer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NativeTcpSocketInitializer")
    }
}
impl NativeTcpSocketInitializer {
    /// The callback must be bounded and return its refusal. It runs before any
    /// TCP connect for each selected address, including later actor reconnects.
    pub fn new(
        initialize: impl for<'a> Fn(NativeTcpSocketRef<'a>) -> io::Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(initialize))
    }
    /// Apply this same trusted policy to the existing HTTP client factory.
    /// Covers its native TCP descriptors, including explicit HTTP proxies.
    /// Requires the source-qualified Reqwest/Hyper socket-input patches.
    pub fn configure_http_client(&self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        let initialize = self.clone();
        builder.native_tcp_socket_initializer(move |socket| {
            (initialize.0)(NativeTcpSocketRef(NativeTcpSocketKind::Http(socket)))
        })
    }

    pub(super) fn initialize(&self, socket: &tokio::net::TcpSocket) -> io::Result<()> {
        (self.0)(NativeTcpSocketRef(NativeTcpSocketKind::Relay(socket)))
    }
}

/// A host-policy refusal is terminal for the current TCP dial. It cannot be
/// treated as ordinary address failure and bypassed by another IP/family.
#[derive(Debug)]
pub struct SocketInitializationError(pub(super) io::Error);
impl fmt::Display for SocketInitializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native TCP socket initialization refused: {}", self.0)
    }
}
impl std::error::Error for SocketInitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
