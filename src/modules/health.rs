use axum::{Json, Router, response::Redirect, routing::get};
use serde_json::{Value, json};

pub fn routes() -> Router {
    Router::new()
        .route("/", get(|| async { Redirect::temporary("/health") }))
        .route("/health", get(health))
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
