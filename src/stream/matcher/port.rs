use http::Request;

use crate::{
    service::{context::Extensions, Context},
    stream::SocketInfo,
};

#[derive(Debug, Clone)]
/// Filter based on the port part of the [`SocketAddr`] of the peer.
///
/// [`SocketAddr`]: std::net::SocketAddr
pub struct PortFilter {
    port: u16,
    optional: bool,
}

impl PortFilter {
    /// create a new port filter to filter on the port part a [`SocketAddr`]
    ///
    /// This filter will not match in case socket address could not be found,
    /// if you want to match in case socket address could not be found,
    /// use the [`PortFilter::optional`] constructor..
    ///
    /// [`SocketAddr`]: std::net::SocketAddr
    pub fn new(port: u16) -> Self {
        Self {
            port,
            optional: false,
        }
    }

    /// create a new port filter to filter on the port part a [`SocketAddr`]
    ///
    /// This filter will match in case socket address could not be found.
    /// Use the [`PortFilter::new`] constructor if you want do not want
    /// to match in case socket address could not be found.
    ///
    /// [`SocketAddr`]: std::net::SocketAddr
    pub fn optional(port: u16) -> Self {
        Self {
            port,
            optional: true,
        }
    }
}

impl<State, Body> crate::service::Matcher<State, Request<Body>> for PortFilter {
    fn matches(
        &self,
        _ext: Option<&mut Extensions>,
        ctx: &Context<State>,
        _req: &Request<Body>,
    ) -> bool {
        ctx.get::<SocketInfo>()
            .map(|info| info.peer_addr().port() == self.port)
            .unwrap_or(self.optional)
    }
}

impl<State, Socket> crate::service::Matcher<State, Socket> for PortFilter
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
            .map(|addr| addr.port() == self.port)
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
        let filter = PortFilter::new(8080);

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

        // test #3: match: test with matching port
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 2], 8080).into()));
        assert!(filter.matches(None, &ctx, &req));

        // test #4: match: test with different ip, same port
        ctx.insert(SocketInfo::new(None, ([127, 0, 0, 1], 8080).into()));
        assert!(filter.matches(None, &ctx, &req));

        // test #5: match: test with missing socket info, but it's seen as optional
        let filter = PortFilter::optional(8080);
        let ctx = Context::default();
        assert!(filter.matches(None, &ctx, &req));
    }

    #[test]
    fn test_port_filter_socket_trait() {
        let filter = PortFilter::new(8080);

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

        // test #2: match: test with correct port
        socket.peer_addr = Some(([127, 0, 0, 2], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #3: match: test with another correct address
        socket.peer_addr = Some(([127, 0, 0, 1], 8080).into());
        assert!(filter.matches(None, &ctx, &socket));

        // test #5: match: test with missing socket info, but it's seen as optional
        let filter = PortFilter::optional(8080);
        socket.peer_addr = None;
        assert!(filter.matches(None, &ctx, &socket));
    }
}
