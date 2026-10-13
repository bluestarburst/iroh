# Native IP UDP socket initializer prototype

The optional `native-udp-socket-initializer` feature adds
`endpoint::Builder::native_udp_socket_initializer`. It configures the UDP sockets
owned by Iroh's native IP transports before bind. It is not a complete native
VPN underlay capability: DNS, relay TCP/QUIC, portmapper and custom carriers do
not pass through this hook.

The implementation reuses upstream Netwatch `BindOptions::configure_socket` at
immutable net-tools revision `87ad510e0749a5c21ce736478d1369449200da44`. Its
SocketState retains the callback in both Connected and Closed states and applies
it before each new socket is bound, including rebind. No additional peer
lifecycle, connection registry, recovery timer or application retry is added.
The normal API leaves the initializer unspecified.

The callback receives `NativeUdpSocketRef` and `NativeUdpIpFamily`. The reference
implements AsFd on Unix and AsSocket on Windows. It borrows the actual unused
socket and must not retain a raw descriptor or expose it to another sender.
The callback must be bounded. Failure propagates from Netwatch; Iroh tags an
initializer failure so optional-family binding cannot silently ignore a
security refusal. A rebind failure keeps Netwatch's socket Closed.

For a Linux embedding host, the authenticated Proxy OS mechanism can be supplied
without giving its helper any SDK credentials:

```rust,ignore
use std::os::fd::AsFd;
let protector = openrtc_proxy_vpn_os::socket_protection::SocketProtectionClient::new(helper_path)?;
let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
    .native_udp_socket_initializer(move |socket, _family| protector.protect(socket.as_fd()))
    .bind()
    .await?;
```

Initializer installation is constructor-only. This does not retrofit already
created endpoints or protect preexisting unrelated sockets. Consumers must not
report full underlay protection from this API or activate a product VPN while
other SDK socket factories are unqualified.

The pinned Netwatch API is upstream but not yet in published 0.19.3. It also
updates its native interface/netlink dependency family and Apple SO_RECV_ANYIF
behavior; the lock records those versions. Source qualification uses the
explicit root Cargo patch. A standalone published consumer cannot rely on the
root patch being inherited. Registry adoption or a complete published SDK
composition remains a release prerequisite.

## Qualification scope

The public builder refusal case uses an explicitly optional binding followed by
a viable required binding. Removing the initializer-error propagation guard
then accepts an endpoint and fails the intended refusal assertion. Restoring
the guard preserves the initializer error before either endpoint is published.

The Linux kernel case inspects received IP_RECVTTL metadata from actual endpoint
traffic. Initial packets and packets to a new receiver/peer after the existing
socket actor's forced major change retain TTL 31. A later callback refusal closes
the replacement path: a third receiver gets no packet. Fresh receiver ports
exclude queued packets from preceding attempts. This exercises bind, actor
rebind and denial through the existing transport implementation; it does not
prove Proxy helper integration, authenticated application traffic or all SDK
carriers.

The source-shaped Mac suite passes 128 tests with one existing ignored; Linux
passes 129 with three existing ignored using two test threads. A fully parallel
Linux run hit the unchanged one-second slow-lookup deadline; the same case
passed alone and in the two-thread suite, with its deadline unchanged. Native
Clippy and WASM compilation pass (the latter has the existing relay unused-import
warning). Source-current SDK adoption and signed release artifacts remain pending.

The existing operator-only real TUN capture and live interface/publication/QNT
refresh/removal cases also pass again against this revision. They preserve the
underlay inventory and encrypted peer identity. These retain their separate
component scope; the new hook has not yet been composed with the Proxy helper
inside an SDK-owned endpoint.
