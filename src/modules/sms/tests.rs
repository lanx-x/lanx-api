use super::{config::SmsConfig, routes_with_config};
use axum::extract::DefaultBodyLimit;
use axum::{
    Json, Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
    routing::post,
};
use serde_json::{Value, json};
use std::time::Duration;
use tower::ServiceExt;

fn service(url: &str) -> Router {
    routes_with_config(SmsConfig {
        url: url.parse().unwrap(),
        connect_timeout: Duration::from_millis(200),
        request_timeout: Duration::from_millis(200),
    })
    .unwrap()
    .layer(DefaultBodyLimit::max(1024 * 1024))
}

fn request(body: &str) -> Request<Body> {
    Request::post("/api/v1/sms")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn mock(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/events", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (url, task)
}

#[tokio::test]
async fn forwards_transformed_data() {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    let router = Router::new().route(
        "/events",
        post(move |Json(body): Json<Value>| async move {
            tx.send(body).await.unwrap();
            StatusCode::CREATED
        }),
    );
    let (url, task) = mock(router).await;
    let response = service(&url)
        .oneshot(request(r#"{"message":"  验证码：123456\n请勿泄露  ","timestamp":"2026-10-20T12:00:00+08:00","sender":"10690000","extra":{"device":"phone","tags":["sms","test"]}}"#))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        rx.recv().await.unwrap(),
        json!({"msg_type":"text","content":{"text":"  验证码：123456\n请勿泄露  \n2026-10-20T12:00:00+08:00\n10690000"}})
    );
    let bytes = to_bytes(response.into_body(), 1024).await.unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["upstream_status"],
        201
    );
    task.abort();
}

#[tokio::test]
async fn rejects_invalid_input_without_forwarding() {
    for (body, status) in [
        (
            r#"{"message":123,"timestamp":"now","sender":"sms"}"#,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            r#"{"message":"hello","timestamp":"now"}"#,
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        ("{", StatusCode::BAD_REQUEST),
    ] {
        let response = service("http://127.0.0.1:1/events")
            .oneshot(request(body))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    let large =
        json!({"message":"x".repeat(1024 * 1024),"timestamp":"now","sender":"sms"}).to_string();
    let response = service("http://127.0.0.1:1/events")
        .oneshot(request(&large))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn upstream_failure_becomes_bad_gateway() {
    let (url, task) = mock(Router::new().route(
        "/events",
        post(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
    ))
    .await;
    let response = service(&url)
        .oneshot(request(
            r#"{"message":"hello","timestamp":"now","sender":"sms"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    task.abort();
}

#[tokio::test]
async fn upstream_timeout_becomes_gateway_timeout() {
    let (url, task) = mock(Router::new().route(
        "/events",
        post(|| async {
            tokio::time::sleep(Duration::from_secs(2)).await;
            StatusCode::OK
        }),
    ))
    .await;
    let response = service(&url)
        .oneshot(request(
            r#"{"message":"hello","timestamp":"now","sender":"sms"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    task.abort();
}
