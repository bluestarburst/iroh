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

Iroh and Iroh relay depend on this directory through ordinary relative path
dependencies. Noq-proto and Noq-udp retain their existing registry dependencies.
This source is carried by a pinned Git revision; publishing Iroh to a registry
would normalize path-plus-version dependencies back to registry packages and
requires a separately fixed registry dependency chain.
