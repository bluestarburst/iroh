//! Mobile-to-remote-laptop network acceptance.
//!
//! These are transport-owner tests. OpenRTC's managed emulator lane separately
//! proves Firebase discovery, scoped admission, and protected application data.
//! Together they model a native phone reaching a remote native laptop without
//! introducing a second connection lifecycle above Iroh.

use std::time::Duration;

use iroh::{TransportAddr, endpoint::Side};
use n0_error::{Result, StackResultExt};
use n0_tracing_test::traced_test;
use patchbay::{
    Firewall, FirewallConfig, IfaceConfig, IpSupport, LinkCondition, LinkDirection, RouterPreset,
};
use testdir::testdir;

use super::util::{
    Pair, PathConnectionExt, is_relayed, lab_with_relay, payload_accept, payload_open, ping_accept,
    ping_open,
};

const TIMEOUT: Duration = Duration::from_secs(35);
const OUTAGE: Duration = Duration::from_secs(5);
const MOBILE_PAYLOAD_BYTES: usize = 256 * 1024;

async fn assert_relay_plateau_open(conn: &iroh::endpoint::Connection) -> Result {
    let stable_id = conn.stable_id();
    for _ in 0..3 {
        assert!(
            is_relayed(conn),
            "cross-family connection left relay unexpectedly"
        );
        ping_open(conn, TIMEOUT).await?;
        assert_eq!(
            conn.stable_id(),
            stable_id,
            "relay plateau replaced the QUIC connection"
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Ok(())
}

async fn assert_relay_plateau_accept(conn: &iroh::endpoint::Connection) -> Result {
    let stable_id = conn.stable_id();
    for _ in 0..3 {
        ping_accept(conn, TIMEOUT).await?;
        assert!(
            is_relayed(conn),
            "cross-family connection left relay unexpectedly"
        );
        assert_eq!(
            conn.stable_id(),
            stable_id,
            "relay plateau replaced the QUIC connection"
        );
    }
    Ok(())
}

/// Models an IPv6-only cellular phone and an IPv4-only Wi-Fi laptop. The
/// peers have no common direct address family, so the dual-stack relay is the
/// required encrypted bridge.
#[tokio::test]
#[traced_test]
async fn mobile_ipv6_phone_to_ipv4_laptop_uses_stable_relay() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let carrier = lab
        .add_router("carrier_v6")
        .preset(RouterPreset::IspV6)
        .build()
        .await?;
    let wifi = lab
        .add_router("wifi_v4")
        .preset(RouterPreset::Home)
        .ip_support(IpSupport::V4Only)
        .build()
        .await?;
    let phone = lab
        .add_device("phone")
        .iface(
            "wwan0",
            IfaceConfig::routed(carrier.id())
                .condition(LinkCondition::Mobile4G, LinkDirection::Both),
        )
        .build()
        .await?;
    let laptop = lab
        .add_device("laptop")
        .iface(
            "wlan0",
            IfaceConfig::routed(wifi.id()).condition(LinkCondition::Wifi, LinkDirection::Both),
        )
        .build()
        .await?;

    Pair::new(relay_map)
        .server(laptop, async |_dev, _ep, conn| {
            assert_relay_plateau_accept(&conn).await?;
            conn.closed().await;
            Ok(())
        })
        .client(phone, async |_dev, _ep, conn| {
            assert_relay_plateau_open(&conn).await?;
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}

/// Reverses both the address-family asymmetry and QUIC initiator direction:
/// the phone is IPv4-only behind carrier NAT while the laptop is IPv6-only.
#[tokio::test]
#[traced_test]
async fn mobile_ipv4_phone_to_ipv6_laptop_uses_stable_relay() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let carrier = lab
        .add_router("carrier_v4")
        .preset(RouterPreset::IspCgnat)
        .ip_support(IpSupport::V4Only)
        .build()
        .await?;
    let remote_v6 = lab
        .add_router("remote_v6")
        .preset(RouterPreset::IspV6)
        .build()
        .await?;
    let phone = lab
        .add_device("phone")
        .iface(
            "wwan0",
            IfaceConfig::routed(carrier.id())
                .condition(LinkCondition::Mobile4G, LinkDirection::Both),
        )
        .build()
        .await?;
    let laptop = lab
        .add_device("laptop")
        .uplink(remote_v6.id())
        .build()
        .await?;

    Pair::new(relay_map)
        .server(phone, async |_dev, _ep, conn| {
            assert_relay_plateau_accept(&conn).await?;
            conn.closed().await;
            Ok(())
        })
        .client(laptop, async |_dev, _ep, conn| {
            assert_relay_plateau_open(&conn).await?;
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}

/// Both remote networks are IPv6-only. The connection must promote from its
/// relay bootstrap to native IPv6 QUIC and carry application traffic directly.
#[tokio::test]
#[traced_test]
async fn mobile_ipv6_phone_to_ipv6_laptop_promotes_direct() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let carrier = lab
        .add_router("carrier_v6")
        .preset(RouterPreset::IspV6)
        .build()
        .await?;
    let remote = lab
        .add_router("remote_v6")
        .preset(RouterPreset::IspV6)
        .build()
        .await?;
    let phone = lab
        .add_device("phone")
        .iface(
            "wwan0",
            IfaceConfig::routed(carrier.id())
                .condition(LinkCondition::Mobile4G, LinkDirection::Both),
        )
        .build()
        .await?;
    let laptop = lab.add_device("laptop").uplink(remote.id()).build().await?;

    Pair::new(relay_map)
        .server(laptop, async |_dev, _ep, conn| {
            let selected = conn.wait_ip(TIMEOUT).await?;
            assert!(matches!(selected, TransportAddr::Ip(addr) if addr.ip().is_ipv6()));
            ping_accept(&conn, TIMEOUT).await?;
            conn.closed().await;
            Ok(())
        })
        .client(phone, async |_dev, _ep, conn| {
            let stable_id = conn.stable_id();
            let selected = conn.wait_ip(TIMEOUT).await?;
            assert!(matches!(selected, TransportAddr::Ip(addr) if addr.ip().is_ipv6()));
            ping_open(&conn, TIMEOUT).await?;
            assert_eq!(conn.stable_id(), stable_id);
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}

/// Models Wi-Fi Assist or a user leaving Wi-Fi coverage. The phone changes
/// from IPv4 Wi-Fi to IPv6-only cellular while the same logical QUIC
/// connection and its protected streams remain usable.
#[tokio::test]
#[traced_test]
async fn mobile_wifi_ipv4_to_cellular_ipv6_handoff_keeps_connection() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let wifi = lab
        .add_router("phone_wifi")
        .preset(RouterPreset::Home)
        .ip_support(IpSupport::V4Only)
        .build()
        .await?;
    let carrier = lab
        .add_router("carrier_v6")
        .preset(RouterPreset::IspV6)
        .build()
        .await?;
    let laptop_net = lab
        .add_router("laptop_wifi")
        .preset(RouterPreset::Home)
        .ip_support(IpSupport::DualStack)
        .build()
        .await?;
    let phone = lab
        .add_device("phone")
        .iface(
            "en0",
            IfaceConfig::routed(wifi.id()).condition(LinkCondition::Wifi, LinkDirection::Both),
        )
        .build()
        .await?;
    let laptop = lab
        .add_device("laptop")
        .uplink(laptop_net.id())
        .build()
        .await?;

    Pair::new(relay_map)
        .left(Side::Client, phone, async move |dev, ep, conn| {
            let stable_id = conn.stable_id();
            conn.wait_selected(
                TIMEOUT,
                |path| matches!(path.remote_addr(), TransportAddr::Ip(addr) if addr.ip().is_ipv4()),
            )
            .await?;
            ping_open(&conn, TIMEOUT).await?;

            dev.iface("en0").unwrap().replug(carrier.id()).await?;
            ep.network_change().await;
            conn.wait_selected(
                TIMEOUT,
                |path| matches!(path.remote_addr(), TransportAddr::Ip(addr) if addr.ip().is_ipv6()),
            )
            .await
            .context("phone did not migrate to IPv6 cellular")?;
            ping_open(&conn, TIMEOUT).await?;
            assert_eq!(
                conn.stable_id(),
                stable_id,
                "family handoff replaced the connection"
            );
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .right(laptop, async |_dev, _ep, conn| {
            ping_accept(&conn, TIMEOUT).await?;
            ping_accept(&conn, TIMEOUT).await?;
            conn.closed().await;
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}

/// A full radio outage must be recoverable without replacing the existing
/// connection object when the phone returns to the same carrier network.
#[tokio::test]
#[traced_test]
async fn mobile_radio_outage_recovers_same_connection() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let carrier = lab
        .add_router("carrier")
        .preset(RouterPreset::Home)
        .build()
        .await?
        .id();
    let radio_blackhole = lab
        .add_router("radio_blackhole")
        .preset(RouterPreset::Public)
        .firewall(Firewall::Custom(
            FirewallConfig::builder()
                .block_inbound()
                .block_tcp()
                .block_udp()
                .build(),
        ))
        .build()
        .await?
        .id();
    let wifi = lab
        .add_router("wifi")
        .preset(RouterPreset::Home)
        .build()
        .await?;
    let phone = lab.add_device("phone").uplink(carrier).build().await?;
    let laptop = lab.add_device("laptop").uplink(wifi.id()).build().await?;

    Pair::new(relay_map)
        .left(Side::Client, phone, async move |dev, _ep, conn| {
            let stable_id = conn.stable_id();
            conn.wait_ip(TIMEOUT).await?;
            ping_open(&conn, TIMEOUT).await?;
            dev.iface("eth0").unwrap().replug(radio_blackhole).await?;
            tokio::time::sleep(OUTAGE).await;
            dev.iface("eth0").unwrap().replug(carrier).await?;
            ping_open(&conn, TIMEOUT).await?;
            assert_eq!(
                conn.stable_id(),
                stable_id,
                "radio recovery replaced the connection"
            );
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .right(laptop, async |_dev, _ep, conn| {
            ping_accept(&conn, TIMEOUT).await?;
            ping_accept(&conn, TIMEOUT).await?;
            conn.closed().await;
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}

/// iOS IPv6 links must support the minimum IPv6 MTU (1280 bytes). A degraded
/// 3G path carrying a much larger stream catches fixed-1500-byte assumptions,
/// PMTU stalls, and timeout policies tuned only for desktop Wi-Fi.
#[tokio::test]
#[traced_test]
async fn mobile_ipv6_low_mtu_lossy_link_carries_large_payload() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let carrier = lab
        .add_router("carrier_v6")
        .preset(RouterPreset::IspV6)
        .mtu(1280)
        .build()
        .await?;
    let remote = lab
        .add_router("remote_v6")
        .preset(RouterPreset::IspV6)
        .mtu(1280)
        .build()
        .await?;
    let phone = lab
        .add_device("phone")
        .mtu(1280)
        .iface(
            "wwan0",
            IfaceConfig::routed(carrier.id())
                .condition(LinkCondition::Mobile3G, LinkDirection::Both),
        )
        .build()
        .await?;
    let laptop = lab
        .add_device("laptop")
        .mtu(1280)
        .uplink(remote.id())
        .build()
        .await?;

    Pair::new(relay_map)
        .server(laptop, async |_dev, _ep, conn| {
            conn.wait_ip(TIMEOUT).await?;
            payload_accept(&conn, MOBILE_PAYLOAD_BYTES, Duration::from_secs(60)).await?;
            conn.closed().await;
            Ok(())
        })
        .client(phone, async |_dev, _ep, conn| {
            let stable_id = conn.stable_id();
            conn.wait_ip(TIMEOUT).await?;
            payload_open(&conn, MOBILE_PAYLOAD_BYTES, Duration::from_secs(60)).await?;
            assert_eq!(conn.stable_id(), stable_id);
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}

/// Enterprise and some carrier networks block outbound UDP. Iroh must retain
/// a usable encrypted WebSocket relay path rather than reporting a direct
/// route or waiting forever for UDP hole punching.
#[tokio::test]
#[traced_test]
async fn mobile_udp_blocked_network_uses_websocket_relay() -> Result {
    let (lab, relay_map, _relay_guard, guard) = lab_with_relay(testdir!()).await?;
    let restricted = lab
        .add_router("restricted_mobile")
        .preset(RouterPreset::Corporate)
        .build()
        .await?;
    let wifi = lab
        .add_router("wifi")
        .preset(RouterPreset::Home)
        .build()
        .await?;
    let phone = lab
        .add_device("phone")
        .iface(
            "wwan0",
            IfaceConfig::routed(restricted.id())
                .condition(LinkCondition::Mobile4G, LinkDirection::Both),
        )
        .build()
        .await?;
    let laptop = lab.add_device("laptop").uplink(wifi.id()).build().await?;

    Pair::new(relay_map)
        .server(laptop, async |_dev, _ep, conn| {
            assert_relay_plateau_accept(&conn).await?;
            conn.closed().await;
            Ok(())
        })
        .client(phone, async |_dev, _ep, conn| {
            assert_relay_plateau_open(&conn).await?;
            conn.close(0u32.into(), b"done");
            Ok(())
        })
        .run()
        .await?;
    guard.ok();
    Ok(())
}
