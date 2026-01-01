use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::commands::models::Command;
use crate::rest::command::{send_command, ApiErrorResponse, ApiSuccessMessage, CommandError};
use crate::rest::state::AppState;
use crate::storage::models::TemplateConfig;

#[utoipa::path(
    get,
    path = "/api/v1/config/{name}",
    description = "Get the configuration for a template including id_field, dynamic_fields, and hashing_algorithm.",
    params(
        ("name" = String, Path, description = "Template name")
    ),
    responses(
        (status = 200, description = "Template configuration", body = TemplateConfig),
        (status = 404, description = "Template not found", body = ApiErrorResponse),
        (status = 503, description = "Handler unavailable", body = ApiErrorResponse)
    ),
    tag = "config"
)]
pub async fn get_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, CommandError> {
    let result = send_command(&state, |tx| Command::GetConfig { name, response: tx }).await?;

    match result {
        Some(config) => Ok((StatusCode::OK, Json(config)).into_response()),
        None => Ok((
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::new("Template not found")),
        )
            .into_response()),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/config/{name}",
    description = "Set the configuration for a template. Includes id_field (which query parameter identifies unique renders), dynamic_fields (auto-generated values), and hashing_algorithm (none, sha512, or yescrypt for hashing generated values).",
    params(
        ("name" = String, Path, description = "Template name")
    ),
    request_body = TemplateConfig,
    responses(
        (status = 200, description = "Configuration set", body = ApiSuccessMessage),
        (status = 400, description = "Template not found", body = ApiErrorResponse),
        (status = 503, description = "Handler unavailable", body = ApiErrorResponse)
    ),
    tag = "config"
)]
pub async fn set_config(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(config): Json<TemplateConfig>,
) -> Result<impl IntoResponse, CommandError> {
    send_command(&state, |tx| Command::SetConfig {
        name,
        config,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiSuccessMessage::new("config set"))))
}
