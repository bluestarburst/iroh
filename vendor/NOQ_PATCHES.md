# Vendored Noq patch provenance

This directory carries the Noq 1.3.0 package from crates.io, with the original
MIT and Apache-2.0 license texts and copyright notices retained in `noq/`.

Upstream source revision: `c1f411562e6078852749b8bcf1190523096a107f`
(package subdirectory `noq`). Published crate SHA-256:
`b78be567e796cfa74bb9bdc4117790af2d505eb887019cf8803244353eb09d89`.

Local changes relative to that package:

- `src/recv_stream.rs` and `src/tests.rs`: preserve consumed clean EOF across a
  final read without turning reset/stop/cancel into successful reads.
- `src/lib.rs`, `src/endpoint.rs`, `src/connection.rs`, and the added
  `src/connection/suspension_tests.rs`: retain endpoint receipt time, process
  expiration before late input or fresh transmission can extend a connection,
  and preserve close notifications and draining. Tests use real in-memory
  packets and a manual runtime clock.
- `Cargo.toml`: standalone workspace marker. `.gitignore`: local lock/target
  exclusions. The generated package manifest is authoritative;
  `Cargo.toml.orig` is retained upstream metadata.
- The crate archive's `Cargo.lock` and `.cargo_vcs_info.json` are not included.
  Dependency resolution is owned by the consuming root lockfile; upstream
  origin is recorded above.

Iroh and Iroh relay depend on these vendored packages through ordinary relative
path dependencies. Noq also selects the same vendored Noq-proto. Noq-udp retains
its existing registry dependency.
This source is carried by a pinned Git revision; publishing Iroh to a registry
would normalize path-plus-version dependencies back to registry packages and
requires a separately fixed registry dependency chain.

## Noq-proto 1.3.0 handshake probe repair

`noq-proto/` carries the exact tested package source from the OpenRTC 2.9.4
repair candidate, including the original MIT and Apache-2.0 license texts. Its
upstream registry archive checksum is
`7c1e5b6fe668491eca022f745a0a9402585626c73a7b839b3424ace15d6a9c8f`.

The existing repair selects a handshake probe only in a packet space whose
encryption keys remain available, clears obsolete loss probes, and declines
packet construction after keys are discarded. No protocol source is changed
by this delivery update. The local standalone workspace marker is retained.
`Cargo.toml.orig` remains upstream metadata; the generated package manifest is
authoritative. Consumer root locks own final resolution.

This Git-source route carries the repair only when a consuming root selects
this exact Iroh revision. Publishing Iroh or OpenRTC to crates.io does not
export root patches or preserve vendored path dependencies automatically.
A registry-only repaired dependency chain remains a separate release gate.
