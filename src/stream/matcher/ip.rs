use http::Request;

use crate::stream::dep::ipnet::{IpNet, Ipv4Net, Ipv6Net};
use crate::{
    service::{context::Extensions, Context},
    stream::SocketInfo,
};

#[derive(Debug, Clone)]
/// Filter based on whether or not the [`IpNet`] contains the [`SocketAddr`] of the peer.
///
/// [`SocketAddr`]: std::net::SocketAddr
pub struct IpNetFilter {
    net: IpNet,
    optional: bool,
}

impl IpNetFilter {
    /// create a new IP network filter to filter on an IP Network.
    ///
    /// This filter will not match in case socket address could not be found,
    /// if you want to match in case socket address could not be found,
    /// use the [`IpNetFilter::optional`] constructor..
    pub fn new(net: impl IntoIpNet) -> Self {
        Self {
            net: net.into_ip_net(),
            optional: false,
        }
    }

    /// create a new IP network filter to filter on an IP network
    ///
    /// This filter will match in case socket address could not be found.
    /// Use the [`IpNetFilter::new`] constructor if you want do not want
    /// to match in case socket address could not be found.
    pub fn optional(net: impl IntoIpNet) -> Self {
        Self {
            net: net.into_ip_net(),
            optional: true,
        }
    }
}

impl<State, Body> crate::service::Matcher<State, Request<Body>> for IpNetFilter {
    fn matches(
        &self,
        _ext: Option<&mut Extensions>,
        ctx: &Context<State>,
        _req: &Request<Body>,
    ) -> bool {
        ctx.get::<SocketInfo>()
            .map(|info| self.net.contains(&IpNet::from(info.peer_addr().ip())))
            .unwrap_or(self.optional)
    }
}

impl<State, Socket> crate::service::Matcher<State, Socket> for IpNetFilter
where
    Socket: crate::stream::Socket,
{
    fn matches(
        &self,
        _ext: Option<&mut Extensions>,
        _ctx: &Context<State>,
        stream: &Socket,
    ) -> bool {
        stream
            .peer_addr()
            .map(|addr| self.net.contains(&IpNet::from(addr.ip())))
            .unwrap_or(self.optional)
    }
}

pub trait IntoIpNet: private::Sealed {
    fn into_ip_net(self) -> IpNet;
}

impl IntoIpNet for Ipv4Net {
    fn into_ip_net(self) -> IpNet {
        IpNet::V4(self)
    }
}

impl IntoIpNet for Ipv6Net {
    fn into_ip_net(self) -> IpNet {
        IpNet::V6(self)
    }
}

impl IntoIpNet for IpNet {
    fn into_ip_net(self) -> IpNet {
        self
    }
}

macro_rules! impl_ip_net_from_ip_addr_into_all {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl IntoIpNet for $ty {
                fn into_ip_net(self) -> IpNet {
                    let ip_addr: std::net::IpAddr = self.into();
                    ip_addr.into()
                }
            }
        )+
    };
}

impl_ip_net_from_ip_addr_into_all!(
    std::net::IpAddr,
    std::net::Ipv4Addr,
    std::net::Ipv6Addr,
    [u16; 8],
    [u8; 16],
    [u8; 4],
);

impl IntoIpNet for String {
    fn into_ip_net(self) -> IpNet {
        self.parse().expect("failed to parse ip network")
    }
}

impl IntoIpNet for &str {
    fn into_ip_net(self) -> IpNet {
        self.parse().expect("failed to parse ip network")
    }
}

mod private {
    use super::*;

    pub trait Sealed {}

    impl Sealed for std::net::IpAddr {}
    impl Sealed for std::net::Ipv4Addr {}
    impl Sealed for std::net::Ipv6Addr {}
    impl Sealed for [u16; 8] {}
    impl Sealed for [u8; 16] {}
    impl Sealed for [u8; 4] {}
    impl Sealed for Ipv4Net {}
    impl Sealed for Ipv6Net {}
    impl Sealed for IpNet {}
    impl Sealed for String {}
    impl Sealed for &str {}
}

#[cfg(test)]
mod test {
    use crate::{http::Body, service::Matcher};
    use std::net::SocketAddr;

    use super::*;

    const SUBNET_IPV4: &str = "192.168.0.0/24";
    const SUBNET_IPV4_VALID_CASES: [&str; 2] = ["192.168.0.0/25", "192.168.0.1"];
    const SUBNET_IPV4_INVALID_CASES: [&str; 2] = ["192.167.0.0/23", "192.168.1.0"];

    const SUBNET_IPV6: &str = "fd00::/16";
    const SUBNET_IPV6_VALID_CASES: [&str; 2] = ["fd00::/17", "fd00::1"];
    const SUBNET_IPV6_INVALID_CASES: [&str; 2] = ["fd01::/15", "fd01::"];

    fn socket_addr_from_case(s: &str) -> SocketAddr {
        if s.contains('/') {
            let ip_net: IpNet = s.parse().unwrap();
            SocketAddr::new(ip_net.addr(), 60000)
        } else {
            let ip_addr: std::net::IpAddr = s.parse().unwrap();
            SocketAddr::new(ip_addr, 60000)
        }
    }

    #[test]
    fn test_socket_filter_http() {
        let filter = IpNetFilter::new([127, 0, 0, 1]);

        let mut ctx = Context::default();
        let req = Request::builder()
            .method("GET")
            .uri("/hello")
            .body(Body::empty())
            .unwrap();

        // test #1: no match: test with no socket info registered
        assert!(!filter.matches(None, &ctx, &req));

        // test #2: no match: test with different socket info (ip addr difference)
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 2], 8080).into()));
        assert!(!filter.matches(None, &ctx, &req));

        // test #3: match: test with correct address
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 1], 8080).into()));
        assert!(filter.matches(None, &ctx, &req));

        // test #4: match: test with missing socket info, but it's seen as optional
        let filter = IpNetFilter::optional([127, 0, 0, 1]);
        let mut ctx = Context::default();
        assert!(filter.matches(None, &ctx, &req));

        // test #5: match: valid ipv4 subnets
        let filter = IpNetFilter::new(SUBNET_IPV4);
        for subnet in SUBNET_IPV4_VALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            ctx.insert(SocketInfo::new(None, addr));
            assert!(
                filter.matches(None, &ctx, &req),
                "valid ipv4 subnets => {} >=? {} ({})",
                SUBNET_IPV4,
                addr,
                subnet
            );
        }

        // test #6: match: valid ipv6 subnets
        let filter = IpNetFilter::new(SUBNET_IPV6);
        for subnet in SUBNET_IPV6_VALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            ctx.insert(SocketInfo::new(None, addr));
            assert!(
                filter.matches(None, &ctx, &req),
                "valid ipv6 subnets => {} >=? {} ({})",
                SUBNET_IPV6,
                addr,
                subnet
            );
        }

        // test #7: match: invalid ipv4 subnets
        let filter = IpNetFilter::new(SUBNET_IPV4);
        for subnet in SUBNET_IPV4_INVALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            ctx.insert(SocketInfo::new(None, addr));
            assert!(
                !filter.matches(None, &ctx, &req),
                "invalid ipv4 subnets => {} >=? {} ({})",
                SUBNET_IPV4,
                addr,
                subnet
            );
        }

        // test #8: match: invalid ipv6 subnets
        let filter = IpNetFilter::new(SUBNET_IPV6);
        for subnet in SUBNET_IPV6_INVALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            ctx.insert(SocketInfo::new(None, addr));
            assert!(
                !filter.matches(None, &ctx, &req),
                "invalid ipv6 subnets => {} >=? {} ({})",
                SUBNET_IPV6,
                addr,
                subnet
            );
        }
    }

    #[test]
    fn test_socket_filter_socket_trait() {
        let filter = IpNetFilter::new([127, 0, 0, 1]);

        let ctx = Context::default();

        struct FakeSocket {
            local_addr: Option<SocketAddr>,
            peer_addr: Option<SocketAddr>,
        }

        impl crate::stream::Socket for FakeSocket {
            fn local_addr(&self) -> std::io::Result<SocketAddr> {
                match &self.local_addr {
                    Some(addr) => Ok(*addr),
                    None => Err(std::io::Error::from(std::io::ErrorKind::AddrNotAvailable)),
                }
            }

            fn peer_addr(&self) -> std::io::Result<SocketAddr> {
                match &self.peer_addr {
                    Some(addr) => Ok(*addr),
                    None => Err(std::io::Error::from(std::io::ErrorKind::AddrNotAvailable)),
                }
            }
        }

        let mut socket = FakeSocket {
            local_addr: None,
            peer_addr: Some(([127, 0, 0, 1], 8081).into()),
        };

        // test #1: no match: test with different socket info (ip addr difference)
        socket.peer_addr = Some(([127, 0, 0, 2], 8080).into());
        assert!(!filter.matches(None, &ctx, &socket));

        // test #2: match: test with correct address
        socket.peer_addr = Some(([127, 0, 0, 1], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #3: match: test with missing socket info, but it's seen as optional
        let filter = IpNetFilter::optional([127, 0, 0, 1]);
        socket.peer_addr = None;
        assert!(filter.matches(None, &ctx, &socket));

        // test #4: match: valid ipv4 subnets
        let filter = IpNetFilter::new(SUBNET_IPV4);
        for subnet in SUBNET_IPV4_VALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            socket.peer_addr = Some(addr);
            assert!(
                filter.matches(None, &ctx, &socket),
                "valid ipv4 subnets => {} >=? {} ({})",
                SUBNET_IPV4,
                addr,
                subnet
            );
        }

        // test #5: match: valid ipv6 subnets
        let filter = IpNetFilter::new(SUBNET_IPV6);
        for subnet in SUBNET_IPV6_VALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            socket.peer_addr = Some(addr);
            assert!(
                filter.matches(None, &ctx, &socket),
                "valid ipv6 subnets => {} >=? {} ({})",
                SUBNET_IPV6,
                addr,
                subnet
            );
        }

        // test #6: match: invalid ipv4 subnets
        let filter = IpNetFilter::new(SUBNET_IPV4);
        for subnet in SUBNET_IPV4_INVALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            socket.peer_addr = Some(addr);
            assert!(
                !filter.matches(None, &ctx, &socket),
                "invalid ipv4 subnets => {} >=? {} ({})",
                SUBNET_IPV4,
                addr,
                subnet
            );
        }

        // test #7: match: invalid ipv6 subnets
        let filter = IpNetFilter::new(SUBNET_IPV6);
        for subnet in SUBNET_IPV6_INVALID_CASES.iter() {
            let addr = socket_addr_from_case(subnet);
            socket.peer_addr = Some(addr);
            assert!(
                !filter.matches(None, &ctx, &socket),
                "invalid ipv6 subnets => {} >=? {} ({})",
                SUBNET_IPV6,
                addr,
                subnet
            );
        }
    }
}
