# Native socket policy dependency graph

Iroh's production HTTP and UDP initializer features must resolve for an external
Cargo root without requiring the application to repeat workspace patches.
A fresh consumer previously failed resolution: registry Reqwest has no
`native-socket-initializer` feature. Cargo ignores patches in dependencies.

Iroh and Iroh Relay now directly reference the existing vendored Reqwest and
Hyper utilities. Reqwest and its unchanged upstream TLS connectors reference
that same Hyper source. The TLS connectors are copied from locked versions
Hyper Rustls 0.27.9 and Hyper TLS 0.6.0 with their licenses; only dependency
paths differ. Iroh directly pins Netwatch's existing pre-bind configurator
revision. Constructor policy, HTTP pooling, TLS, DNS and peer lifecycle remain
owned by their original libraries.

The external root uses a fresh lockfile and no patches. Native compilation
passed after selecting `Sink<Bytes>` explicitly in the existing QUIC relay
frame adapter to support the current Tokio codec's additional encoder type.
A fresh external root with all Iroh features enabled and an external WASM
build also passed. PKARR forwarding/refusal tests, HTTPS report/proxy tests
and warnings-denied relay Clippy passed. See the accompanying provenance JSON for hashes.

This repairs Git source consumption. It does **not** make crates.io publication
ready: Cargo rewrites path-plus-version dependencies to registry dependencies
when packaging, and the modified HTTP crates still need a distributable
published identity or an upstream release. OpenRTC's own patched Noq graph
and the remaining VPN transport/platform qualification stay separate gates.
The canonical semver CI job must run again against this source; its earlier
missing-feature failure is preserved, not waived.
