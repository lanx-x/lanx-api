use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
mod config;
#[cfg(test)]
mod tests;
use config::SmsConfig;

#[derive(Clone)]
struct SmsState {
    client: reqwest::Client,
    upstream_url: reqwest::Url,
}

pub fn routes() -> Result<Router, Box<dyn std::error::Error>> {
    routes_with_config(SmsConfig::from_env()?)
}

fn routes_with_config(config: SmsConfig) -> Result<Router, Box<dyn std::error::Error>> {
    config.validate()?;
    let client = reqwest::Client::builder()
        .timeout(config.request_timeout)
        .connect_timeout(config.connect_timeout)
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let router = Router::new()
        .route("/sms", post(send_sms))
        .with_state(SmsState {
            client,
            upstream_url: config.url,
        })
        .layer(middleware::from_fn(log_request));
    Ok(Router::new().nest("/api/v1", router))
}

async fn log_request(request: Request, next: Next) -> Response {
    let (parts, body) = request.into_parts();
    let body = match to_bytes(body, 1024 * 1024).await {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(method = %parts.method, uri = %parts.uri, headers = ?parts.headers,
                %error, "SMS request body could not be read within 1 MiB limit");
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "request body exceeds limit or could not be read",
            )
                .into_response();
        }
    };
    tracing::info!(method = %parts.method, uri = %parts.uri, headers = ?parts.headers,
        body = %String::from_utf8_lossy(&body), "SMS request received");
    next.run(Request::from_parts(parts, Body::from(body))).await
}

#[derive(Deserialize)]
struct SmsRequest {
    message: String,
    timestamp: String,
    sender: String,
}

#[derive(Serialize)]
struct UpstreamRequest {
    msg_type: &'static str,
    content: TextContent,
}

#[derive(Serialize)]
struct TextContent {
    text: String,
}

impl From<SmsRequest> for UpstreamRequest {
    fn from(input: SmsRequest) -> Self {
        Self {
            msg_type: "text",
            content: TextContent {
                text: [input.message, input.timestamp, input.sender].join("\n"),
            },
        }
    }
}

async fn send_sms(State(state): State<SmsState>, Json(input): Json<SmsRequest>) -> Response {
    let output = UpstreamRequest::from(input);

    match state
        .client
        .post(state.upstream_url)
        .json(&output)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            Json(json!({"status": "forwarded", "upstream_status": response.status().as_u16()}))
                .into_response()
        }
        Ok(response) => {
            tracing::warn!(status = %response.status(), "upstream rejected request");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "upstream rejected request"})),
            )
                .into_response()
        }
        Err(error) => {
            let (status, message) = if error.is_timeout() {
                (StatusCode::GATEWAY_TIMEOUT, "upstream request timed out")
            } else {
                (StatusCode::BAD_GATEWAY, "upstream unavailable")
            };
            tracing::warn!(timeout = error.is_timeout(), "upstream request failed");
            (status, Json(json!({"error": message}))).into_response()
        }
    }
}
