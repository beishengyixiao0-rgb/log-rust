use axum::{routing::get, Json, Router};

use serde_json::json;

use crate::stats::GLOBAL_STATS;

async fn metrics() -> Json<serde_json::Value> {
    Json(json!({

        "total":
            GLOBAL_STATS.total.load(
                std::sync::atomic::Ordering::Relaxed
            ),

        "qps":
            GLOBAL_STATS.qps(),

        "error_rate":
            GLOBAL_STATS.error_rate(),
    }))
}

pub async fn start_dashboard() {
    let app = Router::new().route("/metrics", get(metrics));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();

    println!("Dashboard running on :8080");

    axum::serve(listener, app).await.unwrap();
}
