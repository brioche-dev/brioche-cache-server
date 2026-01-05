use clap::Parser;
use miette::IntoDiagnostic as _;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

#[derive(Debug, Clone, Parser)]
struct Args {
    #[arg(long, default_value = "0.0.0.0:3000")]
    bind: String,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let args = Args::parse();

    const DEFAULT_TRACING_DIRECTIVE: &str = concat!(env!("CARGO_CRATE_NAME"), "=info,warn");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_TRACING_DIRECTIVE)),
        )
        .init();

    let app = app().layer(
        tower::ServiceBuilder::new().layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|req: &axum::http::Request<_>| {
                    let path =
                        if let Some(path) = req.extensions().get::<axum::extract::MatchedPath>() {
                            path.as_str()
                        } else {
                            req.uri().path()
                        };
                    let request_id = uuid::Uuid::new_v4();
                    tracing::info_span!("request", path, %request_id)
                })
                .on_request(|_req: &axum::http::Request<_>, _span: &tracing::Span| {
                    tracing::info!("started request");
                })
                .on_response(
                    |res: &axum::http::Response<_>,
                     latency: std::time::Duration,
                     _span: &tracing::Span| {
                        tracing::info!(
                            latency_secs = latency.as_secs_f32(),
                            response_code = res.status().as_u16(),
                            "finished request",
                        );
                    },
                ),
        ),
    );

    let listener = tokio::net::TcpListener::bind(&args.bind)
        .await
        .into_diagnostic()?;
    let addr = listener.local_addr().into_diagnostic()?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await.into_diagnostic()?;

    Ok(())
}

fn app() -> axum::Router {
    axum::Router::new().route("/", axum::routing::get(|| async { "Hello world!" }))
}
