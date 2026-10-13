//! Opt-in native TCP construction input; the existing dialer owns attempts.
use std::{fmt, io, sync::Arc};

/// Borrowed unused TCP socket. Configure it synchronously before connect;
/// never retain a descriptor or create another sender/lifecycle owner.
#[derive(Clone, Copy, Debug)]
pub struct NativeTcpSocketRef<'a>(pub(super) &'a tokio::net::TcpSocket);

#[cfg(unix)]
impl std::os::fd::AsFd for NativeTcpSocketRef<'_> {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        std::os::fd::AsFd::as_fd(self.0)
    }
}
#[cfg(windows)]
impl std::os::windows::io::AsSocket for NativeTcpSocketRef<'_> {
    fn as_socket(&self) -> std::os::windows::io::BorrowedSocket<'_> {
        std::os::windows::io::AsSocket::as_socket(self.0)
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
    pub(super) fn initialize(&self, socket: &tokio::net::TcpSocket) -> io::Result<()> {
        (self.0)(NativeTcpSocketRef(socket))
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
