use http::Request;

use crate::{
    service::{context::Extensions, Context},
    stream::SocketInfo,
};

#[derive(Debug, Clone)]
/// Filter based on the ip part of the [`SocketAddr`] of the peer,
/// matching only if the ip is a loopback address.
///
/// [`SocketAddr`]: std::net::SocketAddr
pub struct LoopbackFilter {
    optional: bool,
}

impl LoopbackFilter {
    /// create a new loopback filter to filter on the ip part a [`SocketAddr`],
    /// matching only if the ip is a loopback address.
    ///
    /// This filter will not match in case socket address could not be found,
    /// if you want to match in case socket address could not be found,
    /// use the [`LoopbackFilter::optional`] constructor..
    ///
    /// [`SocketAddr`]: std::net::SocketAddr
    pub fn new() -> Self {
        Self { optional: false }
    }

    /// create a new loopback filter to filter on the ip part a [`SocketAddr`],
    /// matching only if the ip is a loopback address or no socket address could be found.
    ///
    /// This filter will match in case socket address could not be found.
    /// Use the [`LoopbackFilter::new`] constructor if you want do not want
    /// to match in case socket address could not be found.
    ///
    /// [`SocketAddr`]: std::net::SocketAddr
    pub fn optional() -> Self {
        Self { optional: true }
    }
}

impl Default for LoopbackFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl<State, Body> crate::service::Matcher<State, Request<Body>> for LoopbackFilter {
    fn matches(
        &self,
        _ext: Option<&mut Extensions>,
        ctx: &Context<State>,
        _req: &Request<Body>,
    ) -> bool {
        ctx.get::<SocketInfo>()
            .map(|info| info.peer_addr().ip().is_loopback())
            .unwrap_or(self.optional)
    }
}

impl<State, Socket> crate::service::Matcher<State, Socket> for LoopbackFilter
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
            .map(|addr| addr.ip().is_loopback())
            .unwrap_or(self.optional)
    }
}

#[cfg(test)]
mod test {
    use crate::{http::Body, service::Matcher};
    use std::net::SocketAddr;

    use super::*;

    #[test]
    fn test_port_filter_http() {
        let filter = LoopbackFilter::new();

        let mut ctx = Context::default();
        let req = Request::builder()
            .method("GET")
            .uri("/hello")
            .body(Body::empty())
            .unwrap();

        // test #1: no match: test with no socket info registered
        assert!(!filter.matches(None, &ctx, &req));

        // test #2: no match: test with network address (ipv4)
        ctx.insert(SocketInfo::new(None, ([192, 168, 0, 1], 8080).into()));
        assert!(!filter.matches(None, &ctx, &req));

        // test #3: no match: test with network address (ipv6)
        ctx.insert(SocketInfo::new(
            None,
            ([1, 1, 1, 1, 1, 1, 1, 1], 8080).into(),
        ));
        assert!(!filter.matches(None, &ctx, &req));

        // test #4: match: test with loopback address (ipv4)
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 1], 8080).into()));
        assert!(filter.matches(None, &ctx, &req));

        // test #5: match: test with another loopback address (ipv4)
        ctx.insert(SocketInfo::new(None, ([127, 3, 2, 1], 8080).into()));
        assert!(filter.matches(None, &ctx, &req));

        // test #6: match: test with loopback address (ipv6)
        ctx.insert(SocketInfo::new(
            None,
            ([0, 0, 0, 0, 0, 0, 0, 1], 8080).into(),
        ));
        assert!(filter.matches(None, &ctx, &req));

        // test #7: match: test with missing socket info, but it's seen as optional
        let filter = LoopbackFilter::optional();
        let ctx = Context::default();
        assert!(filter.matches(None, &ctx, &req));
    }

    #[test]
    fn test_port_filter_socket_trait() {
        let filter = LoopbackFilter::new();

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
            peer_addr: None,
        };

        // test #1: no match: test with no socket info registered
        assert!(!filter.matches(None, &ctx, &socket));

        // test #2: no match: test with network address (ipv4)
        socket.peer_addr = Some(([192, 168, 0, 1], 8080).into());
        assert!(!filter.matches(None, &ctx, &socket));

        // test #3: no match: test with network address (ipv6)
        socket.peer_addr = Some(([1, 1, 1, 1, 1, 1, 1, 1], 8080).into());
        assert!(!filter.matches(None, &ctx, &socket));

        // test #4: match: test with loopback address (ipv4)
        socket.peer_addr = Some(([127, 0, 0, 1], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #5: match: test with another loopback address (ipv4)
        socket.peer_addr = Some(([127, 3, 2, 1], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #6: match: test with loopback address (ipv6)
        socket.peer_addr = Some(([0, 0, 0, 0, 0, 0, 0, 1], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #7: match: test with missing socket info, but it's seen as optional
        let filter = LoopbackFilter::optional();
        socket.peer_addr = None;
        assert!(filter.matches(None, &ctx, &socket));
    }
}
