use rama::{
    http::{server::HttpServer, Body, Request, Response},
    rt::Executor,
    service::{layer::TimeoutLayer, service_fn, Context, ServiceBuilder},
    stream::layer::{BytesRWTrackerHandle, BytesTrackerLayer},
    tcp::server::{TcpListener, TcpSocketInfo},
};
use std::time::Duration;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

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

    let graceful = rama::graceful::Shutdown::default();

    graceful.spawn_task_fn(|guard| async {
        let exec = Executor::graceful(guard.clone());

        TcpListener::bind("127.0.0.1:8080")
            .await
            .expect("bind TCP Listener")
            .serve_graceful(
                guard,
                ServiceBuilder::new()
                    .trace_err()
                    .layer(TimeoutLayer::new(Duration::from_secs(8)))
                    .layer(BytesTrackerLayer::new())
                    .service(HttpServer::auto(exec).service(service_fn(
                        |ctx: Context<()>, req: Request| async move {
                            let socket_info = ctx.extensions().get::<TcpSocketInfo>().unwrap();
                            let tracker = ctx.extensions().get::<BytesRWTrackerHandle>().unwrap();
                            let body = Body::from(format!(
                                "Hello {} @ {}, read: {}, written: {}",
                                socket_info.peer_addr(),
                                req.uri().path(),
                                tracker.read(),
                                tracker.written()
                            ));
                            Ok(Response::new(body))
                        },
                    ))),
            )
            .await;
    });

    graceful
        .shutdown_with_limit(Duration::from_secs(30))
        .await
        .expect("graceful shutdown");
}