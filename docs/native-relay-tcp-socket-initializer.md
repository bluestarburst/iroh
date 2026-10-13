# Native relay TCP socket initializer prototype

The default-off `native-tcp-socket-initializer` feature adds
`endpoint::Builder::native_relay_tcp_socket_initializer`. It configures the
actual TCP socket before connect for native WebSocket relay connections and
HTTP CONNECT proxies. Iroh's existing relay actor retains the constructor input
for later dials; its existing Happy Eyeballs dialer still owns address attempts,
timeouts and cancellation. No application reconnect timer or socket registry
is added.

The callback receives a borrowed `iroh_relay::client::NativeTcpSocketRef`, which
implements `AsFd` on Unix and `AsSocket` on Windows. It must be synchronous,
bounded, and return any refusal. Do not retain the descriptor or use it to send
traffic. With no initializer, the existing `TcpStream::connect` path remains.
With an initializer, the dialer creates Tokio's existing `TcpSocket`, configures
it, then connects. A typed `SocketInitializationError` ends that dial rather
than allowing ordinary address/family fallback to bypass a host-policy refusal.
The relay actor remains the reconnect owner.

A Linux embedding can pass the existing authenticated OS mechanism:

```rust,ignore
use std::os::fd::AsFd;
let protector = openrtc_proxy_vpn_os::socket_protection::SocketProtectionClient::new(helper_path)?;
let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
    .native_relay_tcp_socket_initializer(move |socket| protector.protect(socket.as_fd()))
    .bind()
    .await?;
```

This input is constructor-only. It cannot retrofit an already-bound endpoint.
It covers neither DNS resolution sockets, QUIC relay sockets, portmapper,
coordination gateways nor custom carriers. A product must not enable a VPN or
claim complete underlay protection using this partial feature alone. OpenRTC
SDK constructor adoption and actual helper-to-SDK relay composition remain
separate qualification work. No dependency version or published artifact is
changed by this prototype.

## Behavioral proof

The relay dialer test uses a real TCP listener and two resolved addresses. The
first ordinary dial fails; the second exchanges bytes. Both actual descriptors
are configured before connect, and the resulting connected stream retains the
kernel TTL. The public relay-client refusal case covers direct relay and HTTP
proxy configurations in one table: refusal on the first address returns its
policy error and makes no connection to the viable alternative.

The existing `endpoint_send_relay` owner case now also exercises the optional
constructor input. In the feature lane both endpoints have no IP transports,
configure their actual relay TCP descriptors before connect, and exchange an
encrypted bidirectional stream through a local relay. This protects endpoint
builder/relay-actor wiring that standalone relay client checks cannot reach.
TLS verification is disabled only for the existing local self-signed fixture.

Current Mac source-shaped Iroh library suite: 128 passed, one existing ignored.
Linux: 129 passed, three existing ignored, with two test threads. Relay library
suites pass on both platforms with the feature off (7 tests) and on (9 tests),
plus four doctests per lane. Warnings-denied native library Clippy passes on both
platforms. Browser WASM compilation with the two native initializer features
also passes; the existing relay unused-import warning remains.

Executed negative controls in an isolated source copy removed the terminal
refusal guard and the relay actor's initializer forwarding separately. The
former made a connection to the viable alternative and failed the intended
policy-error assertion; the latter exchanged encrypted relay data but failed
the endpoint policy invocation assertion. Both pass again after restoration.

The first Linux full-suite attempt used `--network none`, which cannot satisfy
its existing public DNS check and also stalled the UDP network-change case.
That test container was stopped; the restored-source suite passed with ordinary
container bridge networking and unchanged deadlines. The standalone relay
checks passed in both container configurations. No host route/helper was
installed for this qualification.

See `native-relay-tcp-socket-initializer-evidence.json` for source and log hashes.
Windows runtime, physical independent devices and full SDK socket-factory
coverage remain unverified.


## Network-report HTTP source follow-up

The constructor input now also reaches the existing Reqwest clients used for
relay HTTPS latency and captive-portal probes. Reqwest forwards a default-off
socket input to Hyper's existing TCP factory before bind/connect. Its TLS,
proxy, request, response body, pooling and cancellation code stays in the
upstream libraries. The same borrowed descriptor callback handles both Tokio
relay sockets and Hyper HTTP sockets. A typed refusal terminates Happy Eyeballs
instead of accepting another IP/family. No probe or peer retry owner is added.

The source root and SDK audit mirror contain the pinned HTTP library patches,
with upstream licenses and original package provenance. These root patches are
not inherited by a published dependency. A complete published composition and
source-current SDK adoption remain necessary. The default-off source extension
is not a new shipping VPN capability.

The real network reporter case accepts a configured probe and records relay
latency; refusal yields no successful latency. Public HTTP client tests exercise
fresh connections after clone, pre-bind/pre-connect kernel options, and refusal
for direct/proxy requests across same/mixed IP families. Omitting forwarding,
terminal-refusal handling or report-owner input fails the corresponding intended
assertion, then restored source passes.

Mac Iroh feature suite: 129 passed/one existing ignored. HTTP-feature-off suite:
128 passed/one existing ignored. Linux: 130 passed/three existing ignored, plus
the two existing relay initializer cases. Warnings-denied Iroh/relay Clippy passes
on Mac/Linux. Browser checking passes with the existing relay import warning.
The standalone public-HTTP fixture has its own existing Hyper timer warning.
SDK adoption/helper composition for these new probe sockets, DNS, PKARR/gateway
HTTP, QUIC relay, portmapper, custom carriers, Windows runtime and independent
physical devices remain open. See native-http-probe-socket-policy-evidence.json.
