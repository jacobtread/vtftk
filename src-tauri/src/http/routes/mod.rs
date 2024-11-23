use axum::{
    routing::{get, post},
    Router,
};

mod calibration;
mod data;
mod events;
mod oauth;

pub fn router() -> Router {
    Router::new()
        .route("/oauth", get(oauth::handle_oauth))
        .route("/oauth/complete", post(oauth::handle_oauth_complete))
        .route("/events", get(events::handle_sse))
        .route(
            "/calibration",
            post(calibration::handle_calibration_progress),
        )
        .route("/app-data", get(data::get_app_data))
        .route("/runtime-app-data", get(data::get_runtime_data))
        .route("/runtime-app-data", post(data::set_runtime_data))
}