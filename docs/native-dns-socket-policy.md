# Native DNS socket policy

The existing n0-dns-resolver 0.1.0 sources are reused under vendor, with their
MIT/Apache licenses and source hashes. Opt-in constructor input reaches the
fresh UDP, TCP, DNS-over-TLS and DNS-over-HTTPS socket factories before
bind/connect. The existing DNS query construction, cache, connection pools,
RTT ordering and network reset remain owned by that resolver. Reset retains
the callback through the existing cloned builder. Without the feature or
constructor policy the prior socket paths remain in use.

Host refusal has a typed SocketPolicy error. It is terminal for retransmission,
transport, nameserver, search-name and stale-answer fallback. HTTPS carries a
private error marker through the existing Reqwest/Hyper error chain, then maps
it to the same typed result. Iroh DNS maps this result to its own terminal error
and stops staggered retry and observed family/stream fallback; streaming can
already have yielded a protected address before a concurrent refusal arrives.
It stops on that observation rather than adding a second connection owner.

Use iroh/native-dns-socket-initializer and pass NativeSocketInitializer through
DnsResolver::builder().native_socket_initializer(...), then supply the resolver
to Endpoint::builder().dns_resolver(...). This has not yet been adopted into
the SDK/helper composition. It is constructor API proof, not protected
kernel-route or installed-OS VPN proof.

Two public resolver tests exchange real DNS, TLS and HTTPS messages with local
servers. They cover unused sockets, cached answers, retained TCP/TLS/HTTP pools,
reset and terminal refusal. A separate constructor-omission copy fails by
accepting a forbidden real answer and recording no initializer call. A separate
terminal-fallback omission fails by attempting TCP after UDP policy denial.
Restored source passes. Nine existing transport boundary tests and the Iroh
public staggered-refusal test pass. The vendored test target is included in
non-flaky all-feature Mac, Linux and Windows CI cells, with a committed leaf
lockfile. Windows unbound-socket handling follows Microsoft's getsockname API;
no tests are disabled on Windows.

The prior external-consumer CI now compiles and reaches semver checks. It
identified loss of relay Client Sync through byte-stream type erasure and
shifted DialError positions. Native erasure and private construction retain
Sync, and the new refusal variant is appended. Native all-feature and WASM
compiles pass; the canonical semver gate must rerun. No gate is waived.

Current-source remote DNS/semver results, SDK/helper adoption, independent
devices and native installations remain pending. Normal Proxy VPN is disabled.
Crates.io publication still needs a distributable modified HTTP/DNS cohort.
See native-dns-source-provenance.json for original sources and result hashes.
