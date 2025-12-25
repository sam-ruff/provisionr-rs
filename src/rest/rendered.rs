use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::commands::models::Command;
use crate::rest::command::{send_command, ApiResponse, CommandError};
use crate::rest::state::AppState;
use crate::storage::models::{RenderedTemplate, RenderedTemplateSummary};

#[utoipa::path(
    get,
    path = "/api/rendered/{name}",
    description = "List all rendered instances of a template. Each instance is identified by its ID field value and creation timestamp.",
    params(
        ("name" = String, Path, description = "Template name (with .j2 extension)")
    ),
    responses(
        (status = 200, description = "List of rendered template instances", body = ApiResponse<Vec<RenderedTemplateSummary>>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<Vec<RenderedTemplateSummary>>)
    ),
    tag = "rendered"
)]
pub async fn list_rendered(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, CommandError> {
    let list = send_command(&state, |tx| Command::ListRendered {
        template_name: name,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(list))))
}

#[utoipa::path(
    get,
    path = "/api/rendered/{name}/{id_value}",
    description = "Get a specific rendered template instance including its content and any dynamically generated values.",
    params(
        ("name" = String, Path, description = "Template name (with .j2 extension)"),
        ("id_value" = String, Path, description = "ID field value used when rendering (e.g. MAC address)")
    ),
    responses(
        (status = 200, description = "Rendered template details including content and generated values", body = ApiResponse<RenderedTemplate>),
        (status = 404, description = "Rendered template not found", body = ApiResponse<RenderedTemplate>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<RenderedTemplate>)
    ),
    tag = "rendered"
)]
pub async fn get_rendered(
    State(state): State<AppState>,
    Path((name, id_value)): Path<(String, String)>,
) -> Result<impl IntoResponse, CommandError> {
    let result = send_command(&state, |tx| Command::GetRendered {
        template_name: name,
        id_value,
        response: tx,
    })
    .await?;

    match result {
        Some(rendered) => Ok((StatusCode::OK, Json(ApiResponse::ok(rendered)))),
        None => Ok((StatusCode::NOT_FOUND, Json(ApiResponse::<RenderedTemplate>::not_found()))),
    }
}
