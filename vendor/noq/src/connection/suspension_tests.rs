//! Real endpoint/connection drivers, authenticated packets, and a suspended clock.
//! All I/O stays in memory. No wall-clock waits or Tokio task scheduler are used.

use super::*;
use crate::runtime::AsyncUdpSocket;
use crate::{ClientConfig, Endpoint, EndpointConfig, ServerConfig, TransportConfig};
use std::{
    collections::VecDeque,
    io::IoSliceMut,
    sync::{Mutex as StdMutex, atomic::AtomicBool},
};

type Task = Pin<Box<dyn Future<Output = ()> + Send>>;

struct ManualRuntime {
    now: Arc<StdMutex<Instant>>,
    tasks: StdMutex<Vec<Task>>,
}

impl fmt::Debug for ManualRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ManualRuntime").finish_non_exhaustive()
    }
}

impl ManualRuntime {
    fn new(now: Instant) -> Arc<Self> {
        Arc::new(Self {
            now: Arc::new(StdMutex::new(now)),
            tasks: StdMutex::new(Vec::new()),
        })
    }

    fn advance(&self, dt: Duration) {
        *self.now.lock().unwrap() += dt;
    }

    fn poll_tasks(&self) {
        // Poll existing endpoint before its connection driver, just as when the
        // endpoint's received packet wakes that driver after a renderer resumes.
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap());
        let mut pending = Vec::new();
        for mut task in tasks {
            if task
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
            {
                pending.push(task);
            }
        }
        let mut spawned = self.tasks.lock().unwrap();
        pending.append(&mut *spawned);
        *spawned = pending;
    }

    fn clear(&self) {
        let tasks = std::mem::take(&mut *self.tasks.lock().unwrap());
        drop(tasks);
    }

    fn poll_endpoint_only(&self) {
        let mut endpoint = self.tasks.lock().unwrap().remove(0);
        assert!(
            endpoint
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        self.tasks.lock().unwrap().insert(0, endpoint);
    }
}

#[derive(Debug)]
struct ManualTimer {
    now: Arc<StdMutex<Instant>>,
    deadline: Instant,
}

impl AsyncTimer for ManualTimer {
    fn reset(mut self: Pin<&mut Self>, deadline: Instant) {
        self.deadline = deadline;
    }

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if *self.now.lock().unwrap() >= self.deadline {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

impl Runtime for ManualRuntime {
    fn new_timer(&self, deadline: Instant) -> Pin<Box<dyn AsyncTimer>> {
        Box::pin(ManualTimer {
            now: self.now.clone(),
            deadline,
        })
    }

    fn spawn(&self, future: Task) {
        self.tasks.lock().unwrap().push(future);
    }

    fn wrap_udp_socket(&self, _: std::net::UdpSocket) -> io::Result<Box<dyn AsyncUdpSocket>> {
        panic!("this regression must not create real sockets")
    }

    fn now(&self) -> Instant {
        *self.now.lock().unwrap()
    }
}

#[derive(Debug)]
struct Packet {
    bytes: Vec<u8>,
    remote: SocketAddr,
    ecn: Option<udp::EcnCodepoint>,
}

type Packets = Arc<StdMutex<VecDeque<Packet>>>;

#[derive(Debug, Default)]
struct SendGate {
    blocked: AtomicBool,
    fail: AtomicBool,
    calls: AtomicUsize,
}

#[derive(Debug)]
struct MemorySocket {
    addr: SocketAddr,
    inbound: Packets,
    outbound: Packets,
    gate: Arc<SendGate>,
}

#[derive(Debug)]
struct MemorySender {
    addr: SocketAddr,
    outbound: Packets,
    gate: Arc<SendGate>,
}

impl UdpSender for MemorySender {
    fn poll_send(
        self: Pin<&mut Self>,
        transmit: &udp::Transmit<'_>,
        _: &mut Context<'_>,
    ) -> Poll<io::Result<()>> {
        self.gate.calls.fetch_add(1, Ordering::SeqCst);
        if self.gate.fail.load(Ordering::SeqCst) {
            return Poll::Ready(Err(io::Error::other("test socket write failure")));
        }
        if self.gate.blocked.load(Ordering::SeqCst) {
            return Poll::Pending;
        }
        assert!(transmit.segment_size.is_none());
        self.outbound.lock().unwrap().push_back(Packet {
            bytes: transmit.contents.to_vec(),
            remote: self.addr,
            ecn: transmit.ecn,
        });
        Poll::Ready(Ok(()))
    }
}

impl AsyncUdpSocket for MemorySocket {
    fn create_sender(&self) -> Pin<Box<dyn UdpSender>> {
        Box::pin(MemorySender {
            addr: self.addr,
            outbound: self.outbound.clone(),
            gate: self.gate.clone(),
        })
    }

    fn poll_recv(
        &mut self,
        _: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
        metas: &mut [udp::RecvMeta],
    ) -> Poll<io::Result<usize>> {
        let Some(packet) = self.inbound.lock().unwrap().pop_front() else {
            return Poll::Pending;
        };
        bufs[0][..packet.bytes.len()].copy_from_slice(&packet.bytes);
        let meta = &mut metas[0];
        meta.addr = packet.remote;
        meta.len = packet.bytes.len();
        meta.stride = packet.bytes.len();
        meta.ecn = packet.ecn;
        meta.dst_ip = Some(self.addr.ip());
        Poll::Ready(Ok(1))
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        Ok(self.addr)
    }
}

struct Pair {
    guest: Connection,
    host: Connection,
    guest_runtime: Arc<ManualRuntime>,
    host_runtime: Arc<ManualRuntime>,
    guest_inbound: Packets,
    guest_send_gate: Arc<SendGate>,
    _endpoints: (Endpoint, Endpoint),
}

impl Drop for Pair {
    fn drop(&mut self) {
        self.guest_runtime.clear();
        self.host_runtime.clear();
    }
}

impl Pair {
    fn new() -> Self {
        Self::with_initial_packet_loss(false)
    }

    fn with_initial_packet_loss(drop_initial: bool) -> Self {
        let now = Instant::now();
        let guest_runtime = ManualRuntime::new(now);
        let host_runtime = ManualRuntime::new(now);
        let guest_inbound = Packets::default();
        let host_inbound = Packets::default();
        let guest_send_gate = Arc::new(SendGate::default());
        let guest_addr = "[::1]:10001".parse().unwrap();
        let host_addr = "[::1]:10002".parse().unwrap();
        // Test-only TLS identity, matching the crate's existing endpoint tests.
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert.cert.der().clone()).unwrap();
        let key = rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der());
        let mut server =
            ServerConfig::with_single_cert(vec![cert.cert.der().clone()], key.into()).unwrap();
        let mut transport = TransportConfig::default();
        transport.max_idle_timeout(Some(Duration::from_secs(30).try_into().unwrap()));
        transport.keep_alive_interval(Some(Duration::from_secs(5)));
        let transport = Arc::new(transport);
        server.transport_config(transport.clone());
        let guest_endpoint = Endpoint::new_with_abstract_socket(
            EndpointConfig::default(),
            None,
            Box::new(MemorySocket {
                addr: guest_addr,
                inbound: guest_inbound.clone(),
                outbound: host_inbound.clone(),
                gate: guest_send_gate.clone(),
            }),
            guest_runtime.clone(),
        )
        .unwrap();
        let host_endpoint = Endpoint::new_with_abstract_socket(
            EndpointConfig::default(),
            Some(server),
            Box::new(MemorySocket {
                addr: host_addr,
                inbound: host_inbound.clone(),
                outbound: guest_inbound.clone(),
                gate: Arc::new(SendGate::default()),
            }),
            host_runtime.clone(),
        )
        .unwrap();
        let mut client = ClientConfig::with_root_certificates(Arc::new(roots)).unwrap();
        client.transport_config(transport);
        guest_endpoint.set_default_client_config(client);
        let mut guest_connecting =
            Box::pin(guest_endpoint.connect(host_addr, "localhost").unwrap());
        let mut host_connecting = None;
        let mut guest = None;
        let mut host = None;
        for step in 0..5000 {
            guest_runtime.advance(Duration::from_millis(1));
            host_runtime.advance(Duration::from_millis(1));
            guest_runtime.poll_tasks();
            if drop_initial && step == 0 {
                assert!(!host_inbound.lock().unwrap().is_empty());
                host_inbound.lock().unwrap().clear();
            }
            host_runtime.poll_tasks();
            let mut cx = Context::from_waker(Waker::noop());
            if host_connecting.is_none() && host.is_none() {
                if let Poll::Ready(Some(incoming)) =
                    Box::pin(host_endpoint.accept()).as_mut().poll(&mut cx)
                {
                    host_connecting = Some(Box::pin(incoming.accept().unwrap()));
                }
            }
            if guest.is_none() {
                if let Poll::Ready(result) = guest_connecting.as_mut().poll(&mut cx) {
                    guest = Some(result.unwrap());
                }
            }
            if host.is_none() {
                if let Some(connecting) = host_connecting.as_mut() {
                    if let Poll::Ready(result) = connecting.as_mut().poll(&mut cx) {
                        host = Some(result.unwrap());
                    }
                }
            }
            if guest.is_some() && host.is_some() {
                break;
            }
        }
        let pair = Self {
            guest: guest.expect("guest handshake"),
            host: host.expect("host handshake"),
            guest_runtime,
            host_runtime,
            guest_inbound,
            guest_send_gate,
            _endpoints: (guest_endpoint, host_endpoint),
        };
        for _ in 0..100 {
            pair.guest_runtime.advance(Duration::from_millis(1));
            pair.host_runtime.advance(Duration::from_millis(1));
            pair.guest_runtime.poll_tasks();
            pair.host_runtime.poll_tasks();
        }
        assert!(pair.guest.close_reason().is_none());
        assert!(pair.host.close_reason().is_none());
        pair
    }

    fn hold_authenticated_ack(&self) {
        assert!(self.guest_inbound.lock().unwrap().is_empty());
        self.guest
            .0
            .lock_and_wake("suspension test ping")
            .inner
            .ping();
        self.guest_runtime.poll_tasks();
        // Host answers real encrypted guest traffic; guest stops polling entirely.
        for _ in 0..50 {
            self.host_runtime.advance(Duration::from_millis(1));
            self.host_runtime.poll_tasks();
        }
        assert!(
            !self.guest_inbound.lock().unwrap().is_empty(),
            "must hold a real peer packet"
        );
    }
}

#[test]
fn overdue_idle_expires_before_buffered_authenticated_ack() {
    let pair = Pair::new();
    pair.hold_authenticated_ack();
    let stable_id = pair.guest.stable_id();
    let before_acks = pair.guest.stats().frame_rx.acks;
    let before_rtt = pair.guest.rtt(PathId::ZERO).unwrap();
    // The host continues running while only the guest is suspended.
    for _ in 0..960 {
        pair.host_runtime.advance(Duration::from_secs(1));
        pair.host_runtime.poll_tasks();
    }
    assert!(matches!(
        pair.host.close_reason(),
        Some(ConnectionError::TimedOut)
    ));
    pair.guest_runtime.advance(Duration::from_secs(960));
    let now = pair.guest_runtime.now();
    assert!(
        pair.guest
            .0
            .lock_without_waking("expired deadline")
            .inner
            .poll_timeout()
            .unwrap()
            < now
    );
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    println!(
        "resumed: stable_same={} host={:?} guest={:?} ack_delta={} rtt_before={:?} rtt_after={:?}",
        pair.guest.stable_id() == stable_id,
        pair.host.close_reason(),
        pair.guest.close_reason(),
        pair.guest.stats().frame_rx.acks - before_acks,
        before_rtt,
        pair.guest.rtt(PathId::ZERO)
    );
    assert!(
        matches!(pair.guest.close_reason(), Some(ConnectionError::TimedOut)),
        "buffered authenticated traffic must not revive a physical connection past its idle deadline"
    );
}

#[test]
fn buffered_authenticated_ack_before_idle_deadline_preserves_connection() {
    let pair = Pair::new();
    pair.hold_authenticated_ack();
    let stable_id = pair.guest.stable_id();
    let before_acks = pair.guest.stats().frame_rx.acks;
    pair.guest_runtime.advance(Duration::from_secs(1));
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(pair.guest.close_reason().is_none());
    assert_eq!(pair.guest.stable_id(), stable_id);
    assert!(
        pair.guest.stats().frame_rx.acks > before_acks,
        "must process a genuine ACK"
    );
}

#[test]
fn timely_queued_ack_survives_driver_poll_after_original_idle_deadline() {
    let pair = Pair::new();
    pair.hold_authenticated_ack();
    let before_acks = pair.guest.stats().frame_rx.acks;
    let mut closed = Box::pin(pair.guest.closed());
    let mut on_closed = Box::pin(pair.guest.on_closed());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(closed.as_mut().poll(&mut cx).is_pending());
    assert!(on_closed.as_mut().poll(&mut cx).is_pending());
    // Endpoint receives before the 30 s idle expiry, but the connection driver
    // does not get to consume that event until after the original deadline.
    pair.guest_runtime.advance(Duration::from_millis(29_500));
    pair.guest_runtime.poll_endpoint_only();
    assert!(pair.guest_inbound.lock().unwrap().is_empty());
    assert_eq!(pair.guest.stats().frame_rx.acks, before_acks);
    pair.guest_runtime.advance(Duration::from_millis(700));
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(pair.guest.close_reason().is_none());
    assert!(pair.guest.stats().frame_rx.acks > before_acks);
    assert!(closed.as_mut().poll(&mut cx).is_pending());
    assert!(on_closed.as_mut().poll(&mut cx).is_pending());
}

#[test]
fn overdue_without_input_notifies_registered_close_waiters_once() {
    let pair = Pair::new();
    assert!(pair.guest_inbound.lock().unwrap().is_empty());
    let mut closed = Box::pin(pair.guest.closed());
    let mut on_closed = Box::pin(pair.guest.on_closed());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(closed.as_mut().poll(&mut cx).is_pending());
    assert!(on_closed.as_mut().poll(&mut cx).is_pending());
    // Fresh application output must not reset an already elapsed idle deadline.
    pair.guest
        .0
        .lock_and_wake("pending ping at expiry")
        .inner
        .ping();
    pair.guest_runtime.advance(Duration::from_secs(960));
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(matches!(
        closed.as_mut().poll(&mut cx),
        Poll::Ready(ConnectionError::TimedOut)
    ));
    let Poll::Ready(result) = on_closed.as_mut().poll(&mut cx) else {
        panic!("on_closed not published")
    };
    assert!(matches!(result.reason, ConnectionError::TimedOut));
    assert!(
        pair.guest
            .0
            .lock_without_waking("close waiters delivered")
            .on_closed
            .is_empty()
    );
    // Repeated driver polling must not re-publish the terminal event.
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(matches!(
        pair.guest.close_reason(),
        Some(ConnectionError::TimedOut)
    ));
    assert!(
        pair.guest
            .0
            .lock_without_waking("no duplicate close waiters")
            .on_closed
            .is_empty()
    );
}

#[test]
fn local_close_drains_without_later_idle_overwriting_reason() {
    let pair = Pair::new();
    let mut closed = Box::pin(pair.guest.closed());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(closed.as_mut().poll(&mut cx).is_pending());
    pair.guest.close(0u8.into(), b"local close");
    assert!(matches!(
        closed.as_mut().poll(&mut cx),
        Poll::Ready(ConnectionError::LocallyClosed)
    ));
    pair.guest_runtime.advance(Duration::from_secs(960));
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(matches!(
        pair.guest.close_reason(),
        Some(ConnectionError::LocallyClosed)
    ));
    assert!(
        pair.guest
            .0
            .lock_without_waking("drained close")
            .inner
            .is_drained()
    );
}

#[test]
fn lost_initial_handshake_packet_still_retransmits_and_connects() {
    let pair = Pair::with_initial_packet_loss(true);
    assert!(pair.guest.close_reason().is_none());
    assert!(pair.host.close_reason().is_none());
    assert!(pair.guest.stats().frame_tx.crypto >= 2);
}

#[test]
fn overdue_datagram_then_queued_close_publishes_timeout_consistently() {
    let pair = Pair::new();
    pair.hold_authenticated_ack();
    let mut closed = Box::pin(pair.guest.closed());
    let mut on_closed = Box::pin(pair.guest.on_closed());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(closed.as_mut().poll(&mut cx).is_pending());
    assert!(on_closed.as_mut().poll(&mut cx).is_pending());
    pair.guest_runtime.advance(Duration::from_secs(960));
    pair.guest_runtime.poll_endpoint_only();
    // Queue a local close behind the already received overdue datagram.
    pair._endpoints.0.close(0u8.into(), b"queued local close");
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(matches!(
        closed.as_mut().poll(&mut cx),
        Poll::Ready(ConnectionError::TimedOut)
    ));
    let Poll::Ready(result) = on_closed.as_mut().poll(&mut cx) else {
        panic!("on_closed not published")
    };
    assert!(matches!(result.reason, ConnectionError::TimedOut));
    assert!(matches!(
        pair.guest.close_reason(),
        Some(ConnectionError::TimedOut)
    ));
    assert!(
        pair.guest
            .0
            .lock_without_waking("terminal notified once")
            .on_closed
            .is_empty()
    );
}

#[test]
fn overdue_idle_cancels_buffered_transmit_before_socket_error() {
    let pair = Pair::new();
    let mut closed = Box::pin(pair.guest.closed());
    let mut cx = Context::from_waker(Waker::noop());
    assert!(closed.as_mut().poll(&mut cx).is_pending());
    pair.guest_send_gate.blocked.store(true, Ordering::SeqCst);
    pair.guest.0.lock_and_wake("blocked ping").inner.ping();
    pair.guest_runtime.poll_tasks();
    assert!(
        pair.guest
            .0
            .lock_without_waking("buffered write")
            .buffered_transmit
            .is_some()
    );
    let calls = pair.guest_send_gate.calls.load(Ordering::SeqCst);
    pair.guest_send_gate.fail.store(true, Ordering::SeqCst);
    pair.guest_runtime.advance(Duration::from_secs(960));
    for _ in 0..3 {
        pair.guest_runtime.poll_tasks();
    }
    assert!(matches!(
        closed.as_mut().poll(&mut cx),
        Poll::Ready(ConnectionError::TimedOut)
    ));
    assert_eq!(
        pair.guest_send_gate.calls.load(Ordering::SeqCst),
        calls,
        "a terminal connection must not retry the pending socket write"
    );
}
