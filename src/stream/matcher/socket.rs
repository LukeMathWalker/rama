use http::Request;

use crate::{
    service::{context::Extensions, Context},
    stream::SocketInfo,
};
use std::net::SocketAddr;

#[derive(Debug, Clone)]
/// Filter based on the [`SocketAddr`] of the peer.
pub struct SocketAddressFilter {
    addr: SocketAddr,
    optional: bool,
}

impl SocketAddressFilter {
    /// create a new socket address filter to filter on a socket address
    ///
    /// This filter will not match in case socket address could not be found,
    /// if you want to match in case socket address could not be found,
    /// use the [`SocketAddressFilter::optional`] constructor..
    pub fn new(addr: impl Into<SocketAddr>) -> Self {
        Self {
            addr: addr.into(),
            optional: false,
        }
    }

    /// create a new socket address filter to filter on a socket address
    ///
    /// This filter will match in case socket address could not be found.
    /// Use the [`SocketAddressFilter::new`] constructor if you want do not want
    /// to match in case socket address could not be found.
    pub fn optional(addr: impl Into<SocketAddr>) -> Self {
        Self {
            addr: addr.into(),
            optional: true,
        }
    }
}

impl<State, Body> crate::service::Matcher<State, Request<Body>> for SocketAddressFilter {
    fn matches(
        &self,
        _ext: Option<&mut Extensions>,
        ctx: &Context<State>,
        _req: &Request<Body>,
    ) -> bool {
        ctx.get::<SocketInfo>()
            .map(|info| info.peer_addr() == &self.addr)
            .unwrap_or(self.optional)
    }
}

impl<State, Socket> crate::service::Matcher<State, Socket> for SocketAddressFilter
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
            .map(|addr| addr == self.addr)
            .unwrap_or(self.optional)
    }
}

#[cfg(test)]
mod test {
    use crate::{http::Body, service::Matcher};

    use super::*;

    #[test]
    fn test_socket_filter_http() {
        let filter = SocketAddressFilter::new(([127, 0, 0, 1], 8080));

        let mut ctx = Context::default();
        let req = Request::builder()
            .method("GET")
            .uri("/hello")
            .body(Body::empty())
            .unwrap();

        // test #1: no match: test with no socket info registered
        assert!(!filter.matches(None, &ctx, &req));

        // test #2: no match: test with different socket info (port difference)
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 1], 8081).into()));
        assert!(!filter.matches(None, &ctx, &req));

        // test #3: no match: test with different socket info (ip addr difference)
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 2], 8080).into()));
        assert!(!filter.matches(None, &ctx, &req));

        // test #4: match: test with correct address
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 1], 8080).into()));
        assert!(filter.matches(None, &ctx, &req));

        // test #5: match: test with missing socket info, but it's seen as optional
        let filter = SocketAddressFilter::optional(([127, 0, 0, 1], 8080));
        let ctx = Context::default();
        assert!(filter.matches(None, &ctx, &req));
    }

    #[test]
    fn test_socket_filter_socket_trait() {
        let filter = SocketAddressFilter::new(([127, 0, 0, 1], 8080));

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

        // test #1: no match: test with different socket info (port difference)
        assert!(!filter.matches(None, &ctx, &socket));

        // test #2: no match: test with different socket info (ip addr difference)
        socket.peer_addr = Some(([127, 0, 0, 2], 8080).into());
        assert!(!filter.matches(None, &ctx, &socket));

        // test #3: match: test with correct address
        socket.peer_addr = Some(([127, 0, 0, 1], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #5: match: test with missing socket info, but it's seen as optional
        let filter = SocketAddressFilter::optional(([127, 0, 0, 1], 8080));
        socket.peer_addr = None;
        assert!(filter.matches(None, &ctx, &socket));
    }
}
