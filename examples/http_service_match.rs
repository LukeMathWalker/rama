//! This example demonstrates how to create a web router,
//! without the need of service boxing, as is the case with
//! the use of [`WebService`] as demonstrated in
//! the [`http_web_service_dir_and_api`] example.
//!
//! ```sh
//! cargo run --example http_service_match
//! ```
//!
//! # Expected output
//!
//! The server will start and listen on `:8080`. You can use your browser to interact with the service:
//!
//! ```sh
//! open http://localhost:8080
//! curl -v -X PATCH http://localhost:8080/echo
//! ```
//!
//! You should see a the homepage in your browser.
//! The example will also respond to your request with the method and path of the request as JSON.

// rama provides everything out of the box to build a complete web service.
use rama::{
    http::{
        layer::trace::TraceLayer,
        matcher::{HttpMatcher, PathFilter},
        response::{Html, Json, Redirect},
        server::HttpServer,
        service::web::match_service,
        Request,
    },
    rt::Executor,
    service::ServiceBuilder,
};

use serde_json::json;
/// Everything else we need is provided by the standard library, community crates or tokio.
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::DEBUG.into())
                .from_env_lossy(),
        )
        .init();

    let addr = "127.0.0.1:8080";
    tracing::info!("running service at: {addr}");
    let exec = Executor::default();
    HttpServer::auto(exec)
        .listen(
            addr,
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
            .service(
                    match_service!{
                        HttpMatcher::get("/") => Html(r##"<h1>Home</h1><a href="/echo">Echo Request</a>"##.to_string()),
                        PathFilter::new("/echo") => |req: Request| async move {
                            Json(json!({
                                "method": req.method().as_str(),
                                "path": req.uri().path(),
                            }))
                        },
                        _ => Redirect::temporary("/"),
                    }
                ),
        )
        .await
        .unwrap();
}
