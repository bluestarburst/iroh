# Native IP exclusions for OpenRTC overlays

This experimental extension is based on the OpenRTC-pinned Iroh fork revision
`438661f80090161ced4b7e31ca1f31f94e915379`. It is not an upstream Iroh release or
an enabled Proxy VPN feature. OpenRTC integration and OS-interface qualification
are subsequent work.

Native hosts can provide `Builder::excluded_ip_networks(Vec<ipnet::IpNet>)`
before bind and await `Endpoint::set_excluded_ip_networks` on their existing
endpoint. Empty exclusions preserve the previous native IP behavior. The
existing `addr_filter` remains a publication-only filter.

The socket actor owns ordered updates and acknowledges after applying the
shared policy, refreshing local candidates and notifying existing path actors.
The shared candidate source supplies publication and QNT. Final native UDP
send/receive checks enforce current exclusions, including sender clones made
before an update and IPv4-mapped addresses. A read guard spans the synchronous
kernel send, so a completed policy update fences old queued sends. Incoming
batches are compacted before STUN/QUIC processing.

The existing remote path owner filters eligibility before every explicit path
open and before any configured selector sees current/candidate paths. Policy
changes retire an ineligible selection, refresh QNT, and reuse its existing
path-opening machinery for known allowed IP alternatives. They add no endpoint,
logical peer close, admission owner, reconnect loop or new timer. UDP rejection
uses Iroh's existing no-eligible-transport loss semantics, preserving logical
QUIC sessions when an allowed path remains available.

The host must await exclusion commitment before creating overlay routes, and
retain exclusions until those owned routes have been removed. This input does
not confer peer authorization. OpenRTC still owns admission, protected carriage,
logical lifecycle and recovery. Relay/custom transports are unchanged by this
native-IP extension; their DNS/socket/ICE exclusion boundaries still need an
OpenRTC-owned review before claiming complete underlay protection.

## Evidence on 2026-10-13

Four new owning tests pass under both default native and SDK-shaped
`--no-default-features --features tls-ring,unstable-custom-transports` profiles:

- A pre-update sender clone stops emitting excluded UDP to a real kernel
  receiver, then resumes after clearing the policy. Removing the final send
  guard makes the physical-delivery assertion fail.
- A valid encrypted UDP packet from an unrestricted peer does not reach an
  already authenticated QUIC datagram receiver after exclusions apply. Removing
  ingress enforcement makes the datagram-delivery assertion fail.
- A real client observes server QNT candidates converge through exclude/clear/
  exclude updates and retains the original encrypted connection.
- An IPv4-preferred real peer has a verified selected IPv4 path and a peer-
  advertised IPv6 backup. Policy withdrawal switches the actual selected path
  to IPv6 and preserves encrypted bytes and the same connection stable ID.
  This assertion failed before adding eligibility to the existing path owner.

Warnings-denied native library Clippy passes after supplying three missing
native relay fixture defaults and removing two identity conversions of
`std::io::Error` in the fork's existing relay stream adapter. The WASM library
check passes; one existing relay import warning remains there.

The broader SDK-shaped library suite is not green: candidate 121 passed,
5 failed, 1 ignored. An isolated archive of the pinned baseline, with only the
three missing fixture defaults supplied so its tests can compile, reproduces
exactly the same five failures (117 passed, 5 failed, 1 ignored):

- `address_lookup::tests::concurrent_connects_not_blocked_by_slow_lookup`
- `endpoint::connection::tests::test_paths_watcher`
- `endpoint::tests::endpoint_two_direct_add_relay`
- `endpoint::tests::endpoint_unreachable_relay_direct_connect_succeeds`
- `test_utils::test_transport::tests::test_custom_transport_wins_over_relay`

These failures are not waived or removed. No full-suite, SDK artifact,
registry publication, real TUN, relay/custom exclusion, independent-device or
shipping VPN result is claimed by these local loopback checks.
