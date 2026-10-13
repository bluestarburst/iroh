//! OpenRTC opt-in input to the existing HTTP connector's socket factory.
use std::{fmt, io, sync::Arc};

/// Borrowed unused TCP descriptor. Configure synchronously; do not retain it.
#[derive(Clone, Copy, Debug)]
pub struct NativeTcpSocketRef<'a>(&'a socket2::Socket);
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

/// Trusted bounded socket construction policy, retained across fresh dials.
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
    /// Refusal discards the new descriptor before bind or any TCP connect.
    pub fn new(
        initialize: impl for<'a> Fn(NativeTcpSocketRef<'a>) -> io::Result<()> + Send + Sync + 'static,
    ) -> Self {
        Self(Arc::new(initialize))
    }
    pub(super) fn initialize(&self, socket: &socket2::Socket) -> io::Result<()> {
        (self.0)(NativeTcpSocketRef(socket))
    }
}
#[derive(Debug)]
pub(super) struct SocketInitializationError(pub io::Error);
impl fmt::Display for SocketInitializationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native socket policy refused: {}", self.0)
    }
}
impl std::error::Error for SocketInitializationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}
