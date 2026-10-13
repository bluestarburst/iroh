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


Remote run 38439495955 passed dependency advisories, bans, licenses and sources,
repository formatting, optional-dependency absence and Linux minimal crates.
Its spelling check showed that exclusions require matching complete relative
paths; the glob is corrected. Workflow lint had scanned copied upstream
workflow examples. It now targets all owned workflows/actions and Dependabot
configuration, keeping the complete executable workflow surface under audit.
Fresh lint/spelling and runtime results for the corrected head remain pending.


## Public-fork runner assignment

Run 38439877810 passed both native minimal-crate jobs, but its main Linux and
Windows matrices requested Blacksmith labels and the Mac matrix requested
upstream self-hosted ARM64 labels. Job metadata reported queued with no assigned
runner. These jobs had not begun compilation and must not be described as
running runtime tests.

The reusable runner selector now emits standard GitHub-hosted labels when the
repository owner differs from its configured upstream runner organization.
Linux uses ubuntu-latest; Windows uses windows-latest. The Mac matrix uses
macos-14 (ARM64), and direct Linux Android/patchbay jobs use ubuntu-latest in
forks. All matrix cells and commands remain selected. Upstream organization
routing retains its existing runner fleet. The actual selector shell was
executed for Linux, Windows and Mac inputs: core and all 16 overflow slots had
the correct hosted label, without an upstream availability query. Changed YAML
files parse and git diff --check passes. Fresh remote runner assignment and
build/runtime proof are still required.

The separately credentialed upstream netsim job still requires its own
upstream runner/services; this change does not provision those services or
count a queued external-service lane as passed. Public GitHub standard runner
labels and ARM64 Mac availability were checked against:
https://docs.github.com/en/actions/reference/runners/github-hosted-runners.


## First hosted compilation diagnostics

Run 38441005348 assigns real GitHub-hosted Linux, ARM Mac and Windows runners.
Compilation then fails under warnings-denied HTTP/1-only builds because the
upstream Hyper timer argument is unused without HTTP/2. Its argument is renamed
`_timer` and still forwarded unchanged when HTTP/2 is enabled; both source
mirrors match. Patchbay fixtures now use pinned 0.7's actual Moderate/Strict NAT
variants and wifi()/mobile_3g()/mobile_4g() functions. CI compiler diagnostics
and the installed pinned crate confirm those API names. Android setup requests
platform-tools explicitly, because its pinned action defaults to the removed
`tools` package. Formatting, YAML parsing and diff checks pass; remote rebuilt
checks remain pending.

The semver gate also exposes the known non-transitive root patch limitation:
its generated standalone consumer cannot resolve Reqwest's unpublished
native-socket-initializer feature. That gate remains failed, not waived. A
complete independently resolvable dependency graph is still required before
publication. This is a release blocker rather than a reason to claim a source
workspace is a standalone published library.
