# Native PKARR socket policy source qualification

Endpoint-bound `PkarrPublisherBuilder` and `PkarrResolverBuilder` inherit the
endpoint's existing default-off native TCP constructor policy. The existing
Reqwest/Hyper connector initializes each unused descriptor before connect.
Publisher scheduling, signed record verification, HTTP/TLS/proxy/pool behavior,
DNS, address selection and endpoint lifecycle remain owned by existing code.
Direct standalone PKARR client/build calls have no endpoint input and retain
existing behavior. This is not a claim of protection for arbitrary custom
address-lookup services.

The real-listener test uses a public publisher update and resolver stream. Its
PUT body is verified as a signed packet, then returned by GET and verified as
the expected endpoint data. Both fresh connections reach the descriptor policy
before connect. A refused policy produces resolution failure with no accepted
server connection. Removing both builder forwarding assignments allows the
signed exchange but fails the required policy callback count (zero versus two).
The restored source passes. No new production API or resolver/retry owner is
introduced for this test.

Native feature-on/off and WASM compile checks passed. Native UDP/TCP library
suite passed 123 tests with one existing ignored case before the final private
constructor simplification; focused PKARR cases are rerun after that change.
Warnings-denied production Clippy passes. See the evidence receipt for exact
source/log hashes and any remaining checks. Linux helper composition and SDK
adoption for this PKARR change are not yet qualified. The SDK remains pinned
to the preceding HTTPS-qualified Iroh revision. Proxy VPN remains disabled.

## Next DNS owner

The frozen graph uses `n0-dns-resolver` 0.1.0. Its native factories are:

| Owner | Physical construction |
| --- | --- |
| `resolver/transport.rs::udp_query` | Fresh Tokio UDP bind per query/retransmission |
| `resolver/transport.rs::tcp_query` | Fresh Tokio TCP connect on pool miss |
| `resolver/transport.rs::tls_query` | Fresh Tokio TCP connect before TLS |
| `resolver/transport.rs::build_https_client` | Reqwest 0.13 connector with pinned DoH host addresses |

`resolver.rs` owns UDP pacing, truncation-to-TCP fallback, TCP/TLS pools and
lazy HTTPS client construction. `DnsResolver::reset` clones its Builder and
preserves its cache. A DNS input must therefore be retained in that Builder,
forwarded into every fresh physical factory (including fallback), and retained
across reset. Existing lookup policy, fallback, timers, pooling and cache
semantics must remain in that owner. Iroh DNS's builder/reset wrapper then
passes the same input from endpoint construction. Replacing DNS in Proxy or
adding another retry/polling loop is not the implementation path.

Required DNS qualification includes real UDP/TCP/TLS/DoH requests, actual
pre-bind/pre-connect descriptor observation, refusal without transport/family
bypass, fresh pooled replacements and reset. Those changes and tests have not
yet been implemented. PKARR HTTP protection does not protect DNS resolution.
