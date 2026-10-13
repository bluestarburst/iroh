//! Native IP eligibility shared by address publication, QNT and final UDP I/O.
//! This is a route-local input, not a peer/session or retry owner.
use ipnet::IpNet;
use std::{
    io,
    net::IpAddr,
    sync::{Arc, RwLock, RwLockReadGuard},
};

#[derive(Clone, Debug, Default)]
pub(crate) struct IpTransportPolicy(Arc<RwLock<Vec<IpNet>>>);

impl IpTransportPolicy {
    pub(crate) fn new(excluded: Vec<IpNet>) -> Self {
        Self(Arc::new(RwLock::new(excluded)))
    }

    pub(crate) fn read(&self) -> io::Result<PolicyRead<'_>> {
        self.0
            .read()
            .map(PolicyRead)
            .map_err(|_| io::Error::other("IP transport policy unavailable"))
    }

    pub(crate) fn replace(&self, excluded: Vec<IpNet>) -> io::Result<()> {
        *self
            .0
            .write()
            .map_err(|_| io::Error::other("IP transport policy unavailable"))? = excluded;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct PolicyRead<'a>(RwLockReadGuard<'a, Vec<IpNet>>);

impl PolicyRead<'_> {
    pub(crate) fn allows_path(&self, path: &crate::socket::transports::FourTuple) -> bool {
        match path {
            crate::socket::transports::FourTuple::Ip { remote, local } => {
                self.allows(remote.ip()) && local.is_none_or(|ip| self.allows(ip))
            }
            _ => true,
        }
    }

    pub(crate) fn allows(&self, ip: IpAddr) -> bool {
        let canonical = ip.to_canonical();
        let mapped = match canonical {
            IpAddr::V4(ip) => IpAddr::V6(ip.to_ipv6_mapped()),
            IpAddr::V6(ip) => IpAddr::V6(ip),
        };
        !self
            .0
            .iter()
            .any(|network| network.contains(&canonical) || network.contains(&mapped))
    }
}
