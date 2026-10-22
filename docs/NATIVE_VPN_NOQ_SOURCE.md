# Native VPN Noq dependency composition

Iroh 1.4.1 carries Noq and Noq Proto 1.4.0 through ordinary relative dependencies in this public checkout. Iroh and Iroh Relay resolve the same Noq and Proto. Root patches also cover registry consumers; external Git consumers do not inherit workspace patches.

The registry 1.4.0 sources retain the existing clean ordered EOF/read_to_end repair, receipt-time expiry and suspension behavior, and the discarded-Initial-key PTO repair. Existing regressions remain with their implementation owners. No second runtime, timer or recovery authority is introduced. Both upstream MIT and Apache-2.0 licenses are retained.

NATIVE_VPN_NOQ_SOURCE.json records archive hashes and complete carried source hashes. Qualification for the 1.4 migration is in openrtc-1.4-migration.md. Older source-specific evidence files remain historical, not current qualification. Consumer adoption and publication require their own exact-source checks.
