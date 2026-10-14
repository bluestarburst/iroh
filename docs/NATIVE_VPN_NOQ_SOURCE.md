# Native VPN Noq dependency composition

The OpenRTC native socket-policy candidate now carries Noq 1.3.0 and Noq Proto
1.3.0 through ordinary relative dependencies in this public checkout. Iroh and
Iroh Relay select the same Noq and Proto, and Noq selects that same Proto.
This follows the earlier standalone Noq-carrying fork pattern. Root workspace
patches remain for registry users; downstream Git consumers do not inherit them.

The copied source is byte-identical to the OpenRTC SDK candidate's reviewed
`vendor/noq` and `vendor/noq-proto`, including both upstream MIT and Apache-2.0
licenses. It retains the existing receive-stream EOF and pending-stream-credit
repairs used by Proxy's native packet/control E2E. No protocol or runtime policy
change is introduced by this dependency promotion.

The source inventory is `NATIVE_VPN_NOQ_SOURCE.json`. Public Git fetch plus an
external resolved graph and actual Proxy E2E are still required before this
composition is treated as a shipping dependency. This does not publish OpenRTC
2.9.8 or enable a normal Proxy VPN build.
