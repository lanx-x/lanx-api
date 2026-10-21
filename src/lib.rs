mod config;
mod modules;

use axum::{Router, extract::DefaultBodyLimit};

pub fn app() -> Result<Router, Box<dyn std::error::Error>> {
    Ok(modules::routes()?.layer(DefaultBodyLimit::max(1024 * 1024)))
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;

    #[tokio::test]
    async fn root_redirects_to_health() {
        let response = crate::app()
            .unwrap()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(response.headers()["location"], "/health");
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = crate::app()
            .unwrap()
            .oneshot(Request::get("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap(),
            json!({"status":"ok"})
        );
    }
}
