//! Public builder-to-kernel behavior, using the existing socket actor for rebind.
use std::io;
#[cfg(target_os = "linux")]
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{Endpoint, endpoint::presets};

#[tokio::test]
async fn native_udp_initializer_denial_refuses_endpoint_binding() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let calls = AtomicUsize::new(0);
    let result = Endpoint::builder(presets::Minimal)
        .clear_ip_transports()
        .bind_addr_with_opts(
            "127.0.0.1:0",
            crate::endpoint::BindOpts::default()
                .set_prefix_len(32)
                .set_is_required(false),
        )
        .unwrap()
        .bind_addr_with_opts(
            "127.0.0.1:0",
            crate::endpoint::BindOpts::default().set_prefix_len(32),
        )
        .unwrap()
        .native_udp_socket_initializer(move |_, _| {
            if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "initializer-refused-before-bind",
                ))
            } else {
                Ok(())
            }
        })
        .bind()
        .await;
    if let Ok(endpoint) = result {
        endpoint.close().await;
        panic!("optional native IP socket configuration must not swallow initializer denial");
    }
    assert!(format!("{:#}", result.unwrap_err()).contains("initializer-refused-before-bind"));
}

#[cfg(target_os = "linux")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_udp_initializer_reaches_kernel_after_actor_rebind_and_denies_replacement() {
    use std::{
        net::{Ipv4Addr, UdpSocket},
        os::fd::{AsFd, AsRawFd},
    };

    use nix::sys::socket::{ControlMessageOwned, MsgFlags, recvmsg, setsockopt, sockopt};

    use crate::{EndpointAddr, SecretKey};

    fn receiver() -> UdpSocket {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        setsockopt(&socket, sockopt::Ipv4RecvTtl, &true).unwrap();
        socket
    }
    fn emitted_ttl(socket: &UdpSocket) -> io::Result<i32> {
        let mut packet = [0; 2048];
        let mut control = nix::cmsg_space!([i32; 1]);
        let mut slices = [io::IoSliceMut::new(&mut packet)];
        let received = recvmsg::<()>(
            socket.as_raw_fd(),
            &mut slices,
            Some(&mut control),
            MsgFlags::empty(),
        )?;
        assert!(
            received.bytes > 0,
            "receive actual endpoint traffic, not fixture bytes"
        );
        for message in received.cmsgs()? {
            if let ControlMessageOwned::Ipv4Ttl(ttl) = message {
                return Ok(ttl);
            }
        }
        Err(io::Error::other("kernel did not report the packet TTL"))
    }
    fn connect_to(endpoint: &Endpoint, receiver: &UdpSocket) -> tokio::task::JoinHandle<()> {
        let endpoint = endpoint.clone();
        let remote = EndpointAddr::new(SecretKey::generate().public())
            .with_ip_addr(receiver.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = endpoint.connect(remote, b"socket-initializer-proof").await;
        })
    }
    let attempts = Arc::new(AtomicUsize::new(0));
    let refuse = Arc::new(AtomicBool::new(false));
    let seen = attempts.clone();
    let denied = refuse.clone();
    let endpoint = Endpoint::builder(presets::Minimal)
        .bind_addr((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .native_udp_socket_initializer(move |socket, family| {
            if family == netwatch::IpFamily::V4 {
                seen.fetch_add(1, Ordering::SeqCst);
            }
            if denied.load(Ordering::SeqCst) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "replacement-refused",
                ));
            }
            if family == netwatch::IpFamily::V4 {
                // The public hook provides the actual unused socket. This
                // temporary duplicate sets a real kernel option, then closes.
                let duplicate = UdpSocket::from(socket.as_fd().try_clone_to_owned()?);
                assert_eq!(
                    duplicate.local_addr()?.port(),
                    0,
                    "initializer ran after bind"
                );
                duplicate.set_ttl(31)?;
            }
            Ok(())
        })
        .bind()
        .await
        .unwrap();

    let first = receiver();
    let connecting = connect_to(&endpoint, &first);
    assert_eq!(
        emitted_ttl(&first).unwrap(),
        31,
        "initial endpoint socket missed configuration"
    );
    connecting.abort();
    let _ = connecting.await;
    let before = attempts.load(Ordering::SeqCst);
    endpoint.inner().unwrap().force_network_change(true).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while attempts.load(Ordering::SeqCst) <= before {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // A new receiver/peer excludes queued datagrams from the old attempt.
    let second = receiver();
    let connecting = connect_to(&endpoint, &second);
    assert_eq!(
        emitted_ttl(&second).unwrap(),
        31,
        "replacement socket lost the retained initializer"
    );
    connecting.abort();
    let _ = connecting.await;

    refuse.store(true, Ordering::SeqCst);
    let before = attempts.load(Ordering::SeqCst);
    endpoint.inner().unwrap().force_network_change(true).await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while attempts.load(Ordering::SeqCst) <= before {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let third = receiver();
    third
        .set_read_timeout(Some(Duration::from_millis(350)))
        .unwrap();
    let connecting = connect_to(&endpoint, &third);
    let error =
        emitted_ttl(&third).expect_err("a refused replacement socket emitted an endpoint packet");
    assert!(matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ));
    connecting.abort();
    let _ = connecting.await;
    endpoint.close().await;
}
