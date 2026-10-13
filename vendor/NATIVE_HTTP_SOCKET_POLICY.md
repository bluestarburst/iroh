# Source-only native HTTP socket policy patches

These directories mirror published crate sources with their upstream notices.
The opt-in input adds bounded synchronous descriptor configuration before the
existing Hyper TCP bind/connect. Reqwest forwards it without replacing TLS,
proxy, request, body, pooling or cancellation implementations. A typed policy
refusal ends the current Happy Eyeballs dial. No peer/retry authority is added.

- hyper-util 0.1.20, MIT; upstream hyperium/hyper-util revision
  b23a13e2b7ee73e15ba008cd9b19dcd2d3861957.
- reqwest 0.13.4, MIT OR Apache-2.0; upstream seanmonstar/reqwest revision
  11489b34eda6d32b15ad4033e62beba2ee401350.

The copied cargo_vcs_info files record the original package provenance, not
an assertion that these locally modified trees are upstream releases. Source
patches are default-off and unpublished. Root Cargo patches are not inherited
by consumers. Iroh's independent source checkout keeps the same audited mirror
for reproducible tests; the SDK mirror must be kept byte-equivalent.

Native Windows runtime, DNS and complete HTTP/gateway/carrier factory coverage
remain unqualified. Product VPN activation and publication remain disabled.
