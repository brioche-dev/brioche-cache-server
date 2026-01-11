use std::sync::Arc;

use clap::Parser;
use figment::providers::Format as _;
use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _};

use crate::store::http::HttpStore;

mod app;
mod config;
mod models;
mod response;
mod store;

#[derive(Debug, Clone, Parser)]
struct Args {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _args = Args::parse();

    let config: config::Config = figment::Figment::new()
        .merge(figment::providers::Toml::file("config.toml"))
        .merge(figment::providers::Env::prefixed("BRIOCHE_CACHE_SERVER_"))
        .extract()?;

    const DEFAULT_TRACING_DIRECTIVE: &str = concat!(env!("CARGO_CRATE_NAME"), "=info,warn");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(DEFAULT_TRACING_DIRECTIVE)),
        )
        .init();
    let store = match config.upstream_store_url.scheme() {
        "http" | "https" => HttpStore::new(config.upstream_store_url.clone()),
        _ => {
            anyhow::bail!(
                "unsupported scheme for upstream store URL: {}",
                config.upstream_store_url
            );
        }
    };
    let state = app::AppState {
        store: Arc::new(store),
    };

    let app = app::router(state).layer(
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

    let listener = tokio::net::TcpListener::bind(&config.bind_address).await?;
    let addr = listener.local_addr()?;
    tracing::info!("listening on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}
