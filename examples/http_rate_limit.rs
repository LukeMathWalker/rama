//! This example expands the `http_listener_hello` example to specifically showcase
//! how you can rate limit your HTTP server.
//!
//! Note you can also rate limit directly on the transport layer directly.
//!
//! # Run the example
//!
//! ```sh
//! cargo run --example http_rate_limit
//! ```
//!
//! # Expected output
//!
//! The server will start and listen on `:8080`. You can use `curl` to interact with the service:
//!
//! ```sh
//! curl -v http://127.0.0.1:8080
//! curl -v http://127.0.0.1:8080/limit/slow
//! ```
//!
//! You should see a response with `HTTP/1.1 200 OK` and a JSON body with the method and path of the request.
//!
//! You can trigger a Rate Limit by opening 3 concurrent requests to `/limit/slow`:
//!
//! ```sh
//! curl -v http://127.0.0.1:8080/limit/slow
//! ```
//!
//! Or easier by running:
//!
//! ```sh
//! curl -v http://127.0.0.1:8080/api/slow
//! ```
//!
//! Consult your ip address to reach your server from another machine connected to the same network.

use std::{convert::Infallible, sync::Arc, time::Duration};

use http::StatusCode;
use rama::{
    error::BoxError,
    http::{matcher::HttpMatcher, response::Json, server::HttpServer, Request},
    rt::Executor,
    service::{
        layer::{limit::policy::ConcurrentPolicy, LimitLayer},
        util::{backoff::ExponentialBackoff, combinators::Either},
        ServiceBuilder,
    },
    stream::matcher::SocketMatcher,
};
use serde_json::json;

#[tokio::main]
async fn main() {
    let exec = Executor::default();

    HttpServer::auto(exec)
        .listen(
            "0.0.0.0:8080",
            ServiceBuilder::new()
                .map_result(|result: Result<Json<_>, BoxError>| match result {
                    Ok(response) => Ok((StatusCode::OK, response)),
                    Err(err) => Ok((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": err.to_string(),
                        })),
                    )),
                })
                .trace_err()
                // using the [`Either`] combinator you can make tree-like structures,
                // to make as complex rate limiting logic as you wish.
                //
                // For more then 2 variants you can use [`Either3`], [`Either4`], and so on.
                // Keep it as simple as possible for your own sanity however...
                .layer(LimitLayer::new(Arc::new(vec![
                    // external addresses are limited to 1 connection at a time,
                    // when choosing to use backoff, they have to be of same type (generic B),
                    // but you can make them also optional to not use backoff for some, while using it for others
                    (
                        HttpMatcher::socket(SocketMatcher::loopback()).negate(),
                        Some(Either::A(ConcurrentPolicy::with_backoff(1, None))),
                    ),
                    // you can also use options for the policy itself, in case you want to disable
                    // the limit for some
                    (HttpMatcher::path("/admin/*"), None),
                    // test path so you can test also rate limiting on an http level
                    // > NOTE: as you can also make your own Matchers you can limit on w/e
                    // > property you want.
                    (
                        HttpMatcher::path("/limit/*"),
                        Some(Either::A(ConcurrentPolicy::with_backoff(
                            2,
                            Some(ExponentialBackoff::default()),
                        ))),
                    ),
                    // this one is the reason why we are using the (Vec<M, P>, P) approach from above,
                    // as we want to have a default policy for all other requests
                    (
                        HttpMatcher::path("/api/*"),
                        Some(Either::B((
                            vec![
                                (
                                    HttpMatcher::path("/api/slow"),
                                    Some(ConcurrentPolicy::with_backoff(
                                        1,
                                        Some(ExponentialBackoff::default()),
                                    )),
                                ),
                                (HttpMatcher::path("/api/fast"), None),
                            ],
                            Some(ConcurrentPolicy::with_backoff(
                                5,
                                Some(ExponentialBackoff::default()),
                            )),
                        ))),
                    ),
                ])))
                .service_fn(|req: Request| async move {
                    if req.uri().path().ends_with("/slow") {
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }
                    Ok::<_, Infallible>(Json(json!({
                        "method": req.method().as_str(),
                        "path": req.uri().path(),
                    })))
                }),
        )
        .await
        .unwrap();
}
