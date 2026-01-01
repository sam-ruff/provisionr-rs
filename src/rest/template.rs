use axum::{
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use std::collections::HashMap;

use crate::commands::models::Command;
use crate::rest::command::{send_command, ApiErrorResponse, ApiSuccessMessage, CommandError};
use crate::rest::state::AppState;

async fn extract_file_content(multipart: &mut Multipart) -> Result<String, String> {
    let field = multipart
        .next_field()
        .await
        .map_err(|e| format!("Failed to read multipart field: {}", e))?
        .ok_or_else(|| "No file uploaded".to_string())?;

    let bytes = field
        .bytes()
        .await
        .map_err(|e| format!("Failed to read field bytes: {}", e))?;

    String::from_utf8(bytes.to_vec()).map_err(|_| "File content is not valid UTF-8".to_string())
}

#[utoipa::path(
    post,
    path = "/api/v1/template/{name}",
    description = "Upload a Jinja2 template file.",
    params(
        ("name" = String, Path, description = "Template name")
    ),
    request_body(content_type = "multipart/form-data", description = "Template file upload"),
    responses(
        (status = 200, description = "Template created/updated", body = ApiSuccessMessage),
        (status = 400, description = "Invalid template syntax or missing file", body = ApiErrorResponse),
        (status = 503, description = "Handler unavailable", body = ApiErrorResponse)
    ),
    tag = "templates"
)]
pub async fn set_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, CommandError> {
    let content = match extract_file_content(&mut multipart).await {
        Ok(content) => content,
        Err(e) => {
            return Ok((StatusCode::BAD_REQUEST, Json(ApiErrorResponse::new(e))).into_response());
        }
    };

    send_command(&state, |tx| Command::SetTemplate {
        name,
        content,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiSuccessMessage::new("template set"))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/v1/template/{name}/values",
    description = "Set default values for template variables. Values are provided as raw YAML or JSON (JSON is valid YAML). These defaults are used when rendering if not overridden by query parameters.",
    params(
        ("name" = String, Path, description = "Template name")
    ),
    request_body(content_type = "text/plain", description = "Raw YAML or JSON content with key-value pairs"),
    responses(
        (status = 200, description = "Values set", body = ApiSuccessMessage),
        (status = 400, description = "Invalid YAML/JSON syntax", body = ApiErrorResponse),
        (status = 503, description = "Handler unavailable", body = ApiErrorResponse)
    ),
    tag = "templates"
)]
pub async fn set_values(
    State(state): State<AppState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<impl IntoResponse, CommandError> {
    let yaml = match String::from_utf8(body.to_vec()) {
        Ok(s) => s,
        Err(_) => {
            return Ok((
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new("Request body is not valid UTF-8")),
            )
                .into_response());
        }
    };

    send_command(&state, |tx| Command::SetValues {
        name,
        yaml,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiSuccessMessage::new("values set"))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/v1/template/{name}",
    description = "Render a template with provided values. If the same ID field value was used before, returns cached content. Query parameters override default values set via /values endpoint.",
    params(
        ("name" = String, Path, description = "Template name"),
        ("mac_address" = Option<String>, Query, description = "Default ID field value (unless id-field is customised). Required for rendering.")
    ),
    responses(
        (status = 200, description = "Rendered template content", body = String),
        (status = 400, description = "Template not found or missing required ID field", body = String),
        (status = 503, description = "Handler unavailable", body = String)
    ),
    tag = "templates"
)]
pub async fn render_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Response {
    match send_command(&state, |tx| Command::RenderTemplate {
        name,
        query_values: params,
        response: tx,
    })
    .await
    {
        Ok(content) => content.into_response(),
        Err(e) => e.into_plain_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/template/{name}",
    description = "Delete a template and its configuration. Note: Previously rendered instances in the database are not deleted.",
    params(
        ("name" = String, Path, description = "Template name to delete")
    ),
    responses(
        (status = 200, description = "Template deleted", body = ApiSuccessMessage),
        (status = 400, description = "Template not found", body = ApiErrorResponse),
        (status = 503, description = "Handler unavailable", body = ApiErrorResponse)
    ),
    tag = "templates"
)]
pub async fn delete_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, CommandError> {
    send_command(&state, |tx| Command::DeleteTemplate { name, response: tx }).await?;

    Ok((StatusCode::OK, Json(ApiSuccessMessage::new("template deleted"))))
}
