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

The final SDK-shaped library suite passes: 126 passed, 0 failed, 1 existing
ignored test. Its initial five failures were also reproduced in an isolated
archive of the pinned baseline (117 passed, the same 5 failed, 1 ignored), with
only the three fixture defaults required to compile the baseline tests.

Four failures came from preferring a relay for every fresh explicit address
input, even when that relay was unreachable and a usable direct/custom path
existed. The existing remote actor now gives an implicit fresh relay preference
only when an already selected cached path is absent from the new explicit
addresses. Explicit caller preferences retain their meaning. Both unreachable-
relay/direct cases, custom-over-relay selection, path watching, and the existing
same-identity restart/new-ALPN regression pass in the full suite.

The lookup concurrency fixture uses real kernel UDP/QUIC with an unchanged
one-second deadline. Removing its auto-advancing paused test clock resolves a
false timeout. Reintroducing the historical blocking per-peer lookup wait in
the socket actor makes that fixture fail at its intended assertion, `online
connect blocked behind hanging lookup`; the negative control was restored.
No assertion, production deadline, recovery timer or test was removed.

No SDK artifact, registry publication, relay/custom exclusion, independent-device
or shipping VPN result is claimed by the preceding loopback checks.


## Real Linux TUN qualification

Two explicit Linux privileged-fixture cases now run through Proxy's existing
root helper and typed unprivileged `HelperClient`, inside a disposable,
network-disabled namespace. The ordinary library suite ignores these two
operator-only cases; the `linux-underlay.mjs` gate invokes both unconditionally
and fails on missing execution, timeout, wrong payload or incomplete cleanup.
No shipping flag, test-only production API or replacement OS adapter is added.

The native carrier case routes a pre-existing sender's small UDP packet through
a real helper-owned TUN and observes its exact bytes/source/destination. Applying
the pool exclusion stops the next payload; clearing restores capture through
that same sender. Observation starts after `poll_send` completes. Removing the
final UDP guard makes the denied payload appear in the actual TUN and fails the
independent receiver assertion; the source was restored afterward. This is raw
native UDP mechanism proof, not an admitted application packet/session claim.

The candidate case establishes an encrypted Iroh peer before TUN creation.
Both peers exclude the virtual pool, while an unrestricted control endpoint
provides the positive address-collection control. The fixture creates the TUN
and adds a new allowed address to its separate dummy underlay interface. That
new allowed address must reach publication and current in-band QNT, while the
TUN address must reach only the unrestricted control. After verified TUN/route
removal, the allowed alias is removed and must retire from QNT. Encrypted stream
bytes and the original connection stable ID survive all phases; exclusions are
cleared only after owned route/interface teardown is verified.

This case failed twice on the pre-fix owner with both published and QNT addresses
stale after actual kernel removal. With an empty relay map, repeated minor
changes can set the same `(None, LinkChangeMinor)` network-report value and
produce no watched report update. The socket actor now refreshes the shared local
candidate source immediately from current interface facts during its existing
network-change transition. The report still owns external mapping refresh;
there is no additional timer, peer close, reconnect loop or registry. The
unchanged five-second removal gate passes after that owning repair.

Final validation: Mac SDK-shaped library suite 126 passed/1 existing ignored;
Linux suite 126 passed/3 ignored, with the two new privileged cases separately
executed and passing. Native warnings-denied library Clippy, WASM compilation
(one existing relay warning), and Proxy's Linux helper-example warnings-denied
Clippy pass. The complete gate verifies preservation of its foreign underlay
interface/default route and restoration of the original namespace inventory.
These tests do not establish independent-device, relay/custom socket, OS service
installation or complete shipping VPN support.
