# OpenRTC fork on Iroh 1.4.1

Upstream: v1.4.1 / d6c6963b9602d1b8dd2ba53b6519e7eb19cc7940. Retained fork input: 2bf4c5ae8ab068e3d680d12f041e56a8e41c3042, which contains Plutonium's 438661f8 input. This is a merge, preserving both ancestries.

## Retained boundaries

- One existing endpoint/path actor; custom carriers, liveness, same-identity replacement and caller path preference remain subordinate. Upstream bounded asynchronous initial sends retain the fork's expiring preferred initial path.
- Native IP exclusions, route reselection, interface observations and pre-bind UDP initialization/rebind remain in their existing owners.
- Relay TCP, DNS and HTTP socket policies remain default-off constructor inputs. PKARR now combines upstream endpoint proxy routing with descriptor initialization.
- Noq/Proto update to 1.4.0 and retain EOF, driver receipt-time expiry and Initial-key PTO fixes. The previous source-specific proof files are historical.
- Rustls 0.23.46 and rustls-webpki 0.103.16 resolve in the actual lock; full cargo audit reports zero vulnerabilities.

## Checks

Production compile with native TCP/DNS policies and custom transports passed. Native Iroh library suite: 146 passed, one pre-existing ignored. Noq library: 44 passed, three pre-existing ignored. Noq Proto: 448 passed.

The initial Iroh run with default warning-only logging passed145 and failed one existing debug-log assertion in same_endpoint_id_relay. The focused case then passed with RUST_LOG=iroh=debug,iroh_relay=debug, and the full suite passed with that required capture. No assertion, timeout or runtime decision was relaxed.

## Test change review

The existing real-loopback resolver_uses_endpoint_proxy also asserts the constructor-policy callback reaches the proxy TCP connection when enabled. A merge can preserve proxy and descriptor behavior separately while dropping their combination; old independent cases cannot catch that. The same public endpoint builder and real proxy listener provide the proof, with no test-only production API. New upstream tests receive existing fork typed inputs; their assertions remain. No physical-device tests or provider delivery ran. SDK/Plutonium composition and independent review are separate qualification.
