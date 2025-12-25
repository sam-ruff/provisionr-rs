use axum::{
    body::Bytes,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use std::collections::HashMap;
use utoipa::ToSchema;

use crate::commands::models::Command;
use crate::rest::command::{send_command, ApiResponse, CommandError};
use crate::rest::state::AppState;
use crate::storage::models::DynamicFieldConfig;

#[derive(Deserialize, ToSchema)]
pub struct SetIdFieldBody {
    id_field: String,
}

#[derive(Deserialize, ToSchema)]
pub struct SetDynamicFieldsBody {
    fields: Vec<DynamicFieldConfig>,
}

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
    path = "/api/template/{name}",
    description = "Upload a Jinja2 template file. The .j2 extension is automatically appended to the name if not provided.",
    params(
        ("name" = String, Path, description = "Template name (without .j2 extension)")
    ),
    request_body(content_type = "multipart/form-data", description = "Template file upload"),
    responses(
        (status = 200, description = "Template created/updated", body = ApiResponse<String>),
        (status = 400, description = "Invalid template syntax or missing file", body = ApiResponse<String>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<String>)
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
            return Ok((StatusCode::BAD_REQUEST, Json(ApiResponse::<String>::error(e))));
        }
    };

    let template_name = if name.ends_with(".j2") {
        name
    } else {
        format!("{}.j2", name)
    };

    send_command(&state, |tx| Command::SetTemplate {
        name: template_name,
        content,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok("template set".to_string()))))
}

#[utoipa::path(
    put,
    path = "/api/template/{name}/values",
    description = "Set default values for template variables. Values are provided as raw YAML or JSON (JSON is valid YAML). These defaults are used when rendering if not overridden by query parameters.",
    params(
        ("name" = String, Path, description = "Template name (with .j2 extension)")
    ),
    request_body(content_type = "text/plain", description = "Raw YAML or JSON content with key-value pairs"),
    responses(
        (status = 200, description = "Values set", body = ApiResponse<String>),
        (status = 400, description = "Invalid YAML/JSON syntax", body = ApiResponse<String>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<String>)
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
                Json(ApiResponse::<String>::error("Request body is not valid UTF-8")),
            ));
        }
    };

    send_command(&state, |tx| Command::SetValues {
        name,
        yaml,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok("values set".to_string()))))
}

#[utoipa::path(
    put,
    path = "/api/template/{name}/id-field",
    description = "Configure which query parameter identifies unique renders. Defaults to 'mac_address'. The ID field value is used to cache rendered templates - same ID returns cached content, different ID triggers new render.",
    params(
        ("name" = String, Path, description = "Template name (with .j2 extension)")
    ),
    request_body = SetIdFieldBody,
    responses(
        (status = 200, description = "ID field set", body = ApiResponse<String>),
        (status = 400, description = "Template not found", body = ApiResponse<String>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<String>)
    ),
    tag = "templates"
)]
pub async fn set_id_field(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<SetIdFieldBody>,
) -> Result<impl IntoResponse, CommandError> {
    send_command(&state, |tx| Command::SetIdField {
        name,
        id_field: body.id_field,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok("id_field set".to_string()))))
}

#[utoipa::path(
    put,
    path = "/api/template/{name}/dynamic-fields",
    description = "Configure fields to be automatically generated on first render. Generated values are cached with the rendered template. Supports 'Alphanumeric' (random string of specified length) and 'Passphrase' (passphrase with specified word count).",
    params(
        ("name" = String, Path, description = "Template name (with .j2 extension)")
    ),
    request_body = SetDynamicFieldsBody,
    responses(
        (status = 200, description = "Dynamic fields configured", body = ApiResponse<String>),
        (status = 400, description = "Template not found", body = ApiResponse<String>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<String>)
    ),
    tag = "templates"
)]
pub async fn set_dynamic_fields(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(body): Json<SetDynamicFieldsBody>,
) -> Result<impl IntoResponse, CommandError> {
    send_command(&state, |tx| Command::SetDynamicFields {
        name,
        fields: body.fields,
        response: tx,
    })
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok("dynamic_fields set".to_string()))))
}

#[utoipa::path(
    get,
    path = "/api/template/{name}",
    description = "Render a template with provided values. If the same ID field value was used before, returns cached content. Query parameters override default values set via /values endpoint.",
    params(
        ("name" = String, Path, description = "Template name (with .j2 extension)"),
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
    path = "/api/template/{name}",
    description = "Delete a template and its configuration. Note: Previously rendered instances in the database are not deleted.",
    params(
        ("name" = String, Path, description = "Template name to delete (with .j2 extension)")
    ),
    responses(
        (status = 200, description = "Template deleted", body = ApiResponse<String>),
        (status = 400, description = "Template not found", body = ApiResponse<String>),
        (status = 503, description = "Handler unavailable", body = ApiResponse<String>)
    ),
    tag = "templates"
)]
pub async fn delete_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<impl IntoResponse, CommandError> {
    send_command(&state, |tx| Command::DeleteTemplate { name, response: tx }).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok("template deleted".to_string()))))
}
