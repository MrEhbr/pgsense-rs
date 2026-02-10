use std::{
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use axum::{Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use serde::Serialize;
use tokio::net::TcpListener;
use tracing::info;

use crate::metrics::Metrics;

#[derive(Clone)]
pub struct ServerState {
    pub ready: Arc<AtomicBool>,
    pub metrics: Metrics,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
}

async fn health() -> impl IntoResponse {
    axum::Json(HealthResponse { status: "ok" })
}

async fn readiness(State(state): State<ServerState>) -> impl IntoResponse {
    if state.ready.load(Ordering::Relaxed) {
        (StatusCode::OK, axum::Json(HealthResponse { status: "ready" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            axum::Json(HealthResponse { status: "not_ready" }),
        )
    }
}

async fn metrics_handler(State(state): State<ServerState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        state.metrics.encode(),
    )
}

pub async fn start(addr: SocketAddr, state: ServerState) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(readiness))
        .route("/metrics", get(metrics_handler))
        .with_state(state);

    let listener = TcpListener::bind(addr).await?;
    info!(%addr, "HTTP server started");

    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let resp = health().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn readiness_when_not_ready() {
        let state = ServerState {
            ready: Arc::new(AtomicBool::new(false)),
            metrics: Metrics::new(),
        };
        let resp = readiness(State(state)).await.into_response();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn readiness_when_ready() {
        let state = ServerState {
            ready: Arc::new(AtomicBool::new(true)),
            metrics: Metrics::new(),
        };
        let resp = readiness(State(state)).await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
