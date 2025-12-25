mod commands;
mod error;
mod generators;
mod rest;
mod statics;
mod storage;
mod templating;
mod threads;

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use axum::{
    response::{Html, IntoResponse},
    routing::{get, post, put},
    Router,
};
use axum_server::Handle;
use log::{debug, info};
use rust_embed::Embed;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::commands::commander::ConcreteCommander;
use crate::commands::models::Command;
use crate::rest::rendered::{get_rendered, list_rendered};
use crate::rest::state::AppState;
use crate::rest::template::{
    delete_template, render_template, set_dynamic_fields, set_id_field, set_template, set_values,
};
use crate::statics::shutdown::{global_cancellation_token, request_shutdown};
use crate::storage::{EvmapTemplateStore, RenderedStore, SqliteRenderedStore};
use crate::templating::MiniJinjaEngine;
use crate::threads::handler::{ConcreteHandler, Handler};

#[derive(OpenApi)]
#[openapi(
    paths(
        rest::template::set_template,
        rest::template::render_template,
        rest::template::delete_template,
        rest::template::set_values,
        rest::template::set_id_field,
        rest::template::set_dynamic_fields,
        rest::rendered::list_rendered,
        rest::rendered::get_rendered,
    ),
    components(schemas(
        storage::models::GeneratorType,
        storage::models::DynamicFieldConfig,
        storage::models::TemplateData,
        storage::models::RenderedTemplate,
        storage::models::RenderedTemplateSummary,
        rest::template::SetIdFieldBody,
        rest::template::SetDynamicFieldsBody,
        rest::command::ApiResponse<String>,
        rest::command::ApiResponse<storage::models::RenderedTemplate>,
        rest::command::ApiResponse<Vec<storage::models::RenderedTemplateSummary>>,
    )),
    tags(
        (name = "templates", description = "Template management endpoints"),
        (name = "rendered", description = "Rendered template retrieval endpoints")
    ),
    info(
        title = "Provisionr API",
        version = "1.0.0",
        description = "REST API for template provisioning with dynamic value generation"
    )
)]
struct ApiDoc;

#[derive(Embed)]
#[folder = "dist/"]
struct Assets;

static INDEX_HTML: &str = include_str!("../dist/index.html");

async fn index() -> impl IntoResponse {
    Html(INDEX_HTML)
}

async fn static_handler(
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                [(axum::http::header::CONTENT_TYPE, mime.as_ref())],
                content.data.into_owned(),
            )
                .into_response()
        }
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    info!("Starting up");

    let port: u16 = env::var("PROVISIONR_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .expect("PORT must be a number");

    let db_path = env::var("PROVISIONR_DB").unwrap_or_else(|_| "provisionr.db".to_string());

    let template_store = EvmapTemplateStore::new();

    let rendered_store =
        SqliteRenderedStore::new(&db_path).expect("Failed to open database");
    rendered_store.init().expect("Failed to initialise database");

    let (tx, rx) = mpsc::channel::<Command>(128);

    let app_state = AppState {
        command_tx: tx.clone(),
    };

    let engine = MiniJinjaEngine::new();
    let commander = ConcreteCommander::new(engine);

    ctrlc::set_handler(move || {
        request_shutdown();
    })
    .expect("Error setting Ctrl-C handler");

    tokio::spawn(async move {
        let mut handler = ConcreteHandler::new(commander, template_store, rendered_store, rx);
        handler.main_loop().await;
    });

    let app = Router::new()
        .route("/", get(index))
        .route(
            "/api/template/{name}",
            post(set_template).get(render_template).delete(delete_template),
        )
        .route("/api/template/{name}/values", put(set_values))
        .route("/api/template/{name}/id-field", put(set_id_field))
        .route("/api/template/{name}/dynamic-fields", put(set_dynamic_fields))
        .route("/api/rendered/{name}", get(list_rendered))
        .route("/api/rendered/{name}/{id_value}", get(get_rendered))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/{*path}", get(static_handler))
        .with_state(app_state);

    let addr: SocketAddr = format!("0.0.0.0:{port}").parse().unwrap();
    let handle: Handle<SocketAddr> = Handle::new();
    info!("Listening on http://{}", addr);

    tokio::spawn(shutdown_axum(global_cancellation_token(), handle.clone()));

    axum_server::bind(addr)
        .handle(handle)
        .serve(app.into_make_service())
        .await
        .unwrap();
    info!("Shutting down");
}

async fn shutdown_axum(token: CancellationToken, handle: Handle<SocketAddr>) {
    token.cancelled().await;
    debug!("Shutting down axum server.");
    handle.graceful_shutdown(Some(Duration::from_secs(10)));
}
