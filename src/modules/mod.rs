mod health;
mod sms;

use axum::Router;

pub fn routes() -> Result<Router, Box<dyn std::error::Error>> {
    Ok(Router::new().merge(health::routes()).merge(sms::routes()?))
}
