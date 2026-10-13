# Native VPN source CI dependency repair

The current draft run 38438394259 found three blocking checks: workspace
formatting, upstream vendor spelling, and dependency policy. Its security check
identified RUSTSEC-2026-0285 in locked Rustls 0.23.41. The upstream fix is
Rustls 0.23.45: https://rustsec.org/advisories/RUSTSEC-2026-0285.html.

All four Iroh/DNS/relay/server manifests now require 0.23.45 and the root lock
selects 0.23.45 with its required WebPKI/AWS-LC dependency upgrades. Unrelated
WASM and other dependency versions are retained. Locked offline dependency
graph resolution passes; no new runtime proof is claimed for this TLS cohort.
The SDK already locked 0.23.45; its public crate manifest minimum is raised to
the same patched floor without changing its locked graph. Published consumers
and previously built binaries are not changed by these source edits.

The dependency source allowlist now names the audited upstream Netwatch
repository; its immutable revision remains pinned in Cargo.toml. No advisory is
ignored. Workspace source imports match the repository's formatting command.
Codespell excludes the two copied upstream library trees rather than changing
upstream comment text; owned source and documentation remain checked. License
and dependency security checks still cover the vendored dependency graph.

Local checks: repository format-check flags, git diff --check, and complete
locked offline graph resolution for the native host. Fresh remote dependency
policy, spelling, compilation and runtime results remain pending. The old
PKARR/HTTP receipts describe the earlier cohort and do not prove runtime
behavior after this dependency upgrade. VPN activation stays disabled.
