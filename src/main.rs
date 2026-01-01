mod commands;
mod error;
mod generators;
mod rest;
mod statics;
mod storage;
mod templating;
mod threads;

use std::collections::HashMap;
use std::fs;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use serde::Deserialize;

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
use crate::rest::config::{get_config, set_config};
use crate::rest::rendered::{get_rendered, list_rendered};
use crate::rest::state::AppState;
use crate::rest::template::{delete_template, render_template, set_template, set_values};
use crate::statics::shutdown::{global_cancellation_token, request_shutdown};
use crate::storage::models::{DynamicFieldConfig, TemplateData};
use crate::storage::{DashMapTemplateStore, RenderedStore, SqliteRenderedStore, TemplateStore};
use crate::templating::MiniJinjaEngine;
use crate::threads::handler::{ConcreteHandler, Handler};

#[derive(Parser, Debug)]
#[command(name = "provisionr")]
#[command(about = "Template provisioning server with dynamic value generation")]
struct Args {
    /// Path to YAML configuration file
    #[arg(long, short)]
    config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long)]
    log_level: Option<String>,

    /// Port to listen on
    #[arg(long, short)]
    port: Option<u16>,

    /// Database path
    #[arg(long)]
    db: Option<String>,
}

fn default_id_field() -> String {
    "mac_address".to_string()
}

#[derive(Debug, Deserialize, Default)]
struct FileTemplateConfig {
    template_path: Option<PathBuf>,
    values_path: Option<PathBuf>,
    #[serde(default = "default_id_field")]
    id_field: String,
    #[serde(default)]
    dynamic_fields: Vec<DynamicFieldConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    log_level: Option<String>,
    port: Option<u16>,
    db: Option<String>,
    #[serde(default)]
    templates: HashMap<String, FileTemplateConfig>,
}

struct Config {
    log_level: String,
    port: u16,
    db: String,
    config_file: Option<PathBuf>,
    templates: HashMap<String, TemplateData>,
}

impl Config {
    fn from_args(args: Args) -> Self {
        let config_dir = args.config.as_ref().and_then(|p| p.parent().map(|d| d.to_path_buf()));

        let file_config = args
            .config
            .as_ref()
            .map(|path| {
                let content = fs::read_to_string(path)
                    .unwrap_or_else(|e| panic!("Failed to read config file {:?}: {}", path, e));
                serde_yaml::from_str::<FileConfig>(&content)
                    .unwrap_or_else(|e| panic!("Failed to parse config file {:?}: {}", path, e))
            })
            .unwrap_or_default();

        let templates = file_config
            .templates
            .into_iter()
            .map(|(name, file_template)| {
                let template_content = file_template
                    .template_path
                    .map(|p| {
                        let path = resolve_path(&config_dir, &p);
                        fs::read_to_string(&path)
                            .unwrap_or_else(|e| panic!("Failed to read template file {:?}: {}", path, e))
                    })
                    .unwrap_or_default();

                let values_yaml = file_template.values_path.map(|p| {
                    let path = resolve_path(&config_dir, &p);
                    fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("Failed to read values file {:?}: {}", path, e))
                });

                let data = TemplateData {
                    template_content,
                    id_field: file_template.id_field,
                    values_yaml,
                    dynamic_fields: file_template.dynamic_fields,
                };

                (name, data)
            })
            .collect();

        Self {
            log_level: args
                .log_level
                .or(file_config.log_level)
                .unwrap_or_else(|| "info".to_string()),
            port: args.port.or(file_config.port).unwrap_or(3000),
            db: args
                .db
                .or(file_config.db)
                .unwrap_or_else(|| "provisionr.db".to_string()),
            config_file: args.config,
            templates,
        }
    }
}

fn resolve_path(config_dir: &Option<PathBuf>, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else if let Some(dir) = config_dir {
        dir.join(path)
    } else {
        path.clone()
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        rest::template::set_template,
        rest::template::render_template,
        rest::template::delete_template,
        rest::template::set_values,
        rest::config::get_config,
        rest::config::set_config,
        rest::rendered::list_rendered,
        rest::rendered::get_rendered,
    ),
    components(schemas(
        storage::models::GeneratorType,
        storage::models::DynamicFieldConfig,
        storage::models::HashingAlgorithm,
        storage::models::TemplateConfig,
        storage::models::TemplateData,
        storage::models::RenderedTemplate,
        storage::models::RenderedTemplateSummary,
        rest::command::ApiErrorResponse,
        rest::command::ApiSuccessMessage,
    )),
    tags(
        (name = "templates", description = "Template management endpoints"),
        (name = "config", description = "Template configuration endpoints"),
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
    let config = Config::from_args(Args::parse());

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&config.log_level))
        .init();

    if let Some(path) = &config.config_file {
        info!("Loaded configuration from {:?}", path);
    } else {
        info!("Using default configuration");
    }

    info!("Starting up");

    let port = config.port;
    let db_path = config.db;

    let mut template_store = DashMapTemplateStore::new();

    for (name, data) in config.templates {
        info!("Loading template '{}' from config", name);
        template_store.init_template(&name, data);
    }

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
            "/api/v1/template/{name}",
            post(set_template).get(render_template).delete(delete_template),
        )
        .route("/api/v1/template/{name}/values", put(set_values))
        .route("/api/v1/config/{name}", get(get_config).put(set_config))
        .route("/api/v1/rendered/{name}", get(list_rendered))
        .route("/api/v1/rendered/{name}/{id_value}", get(get_rendered))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{GeneratorType, HashingAlgorithm};

    fn fixtures_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    #[test]
    fn load_config_with_templates_and_values() {
        let config_path = fixtures_path().join("config_with_templates.yaml");
        let args = Args {
            config: Some(config_path),
            log_level: None,
            port: None,
            db: None,
        };

        let config = Config::from_args(args);

        assert_eq!(config.log_level, "debug");
        assert_eq!(config.port, 8080);
        assert_eq!(config.db, "test.db");
        assert_eq!(config.templates.len(), 1);

        let greeting = config.templates.get("greeting").expect("greeting template should exist");
        assert_eq!(greeting.id_field, "name");
        assert!(greeting.template_content.contains("Hello {{ name }}"));
        assert!(greeting.values_yaml.as_ref().unwrap().contains("name: World"));
        assert!(greeting.dynamic_fields.is_empty());
    }

    #[test]
    fn load_config_template_only_no_values() {
        let config_path = fixtures_path().join("config_template_only.yaml");
        let args = Args {
            config: Some(config_path),
            log_level: None,
            port: None,
            db: None,
        };

        let config = Config::from_args(args);

        assert_eq!(config.templates.len(), 1);

        let simple = config.templates.get("simple").expect("simple template should exist");
        assert_eq!(simple.id_field, "hostname");
        assert!(simple.template_content.contains("Hello {{ name }}"));
        assert!(simple.values_yaml.is_none());
    }

    #[test]
    fn load_config_with_dynamic_fields() {
        let config_path = fixtures_path().join("config_with_dynamic_fields.yaml");
        let args = Args {
            config: Some(config_path),
            log_level: None,
            port: None,
            db: None,
        };

        let config = Config::from_args(args);

        let kickstart = config.templates.get("kickstart").expect("kickstart template should exist");
        assert_eq!(kickstart.id_field, "mac_address");
        assert_eq!(kickstart.dynamic_fields.len(), 2);

        let root_password = &kickstart.dynamic_fields[0];
        assert_eq!(root_password.field_name, "root_password");
        assert_eq!(root_password.generator_type, GeneratorType::Passphrase { word_count: 4 });
        assert_eq!(root_password.hashing_algorithm, HashingAlgorithm::Sha512);

        let api_key = &kickstart.dynamic_fields[1];
        assert_eq!(api_key.field_name, "api_key");
        assert_eq!(api_key.generator_type, GeneratorType::Alphanumeric { length: 32 });
        assert_eq!(api_key.hashing_algorithm, HashingAlgorithm::None);
    }

    #[test]
    fn load_config_with_multiple_templates() {
        let config_path = fixtures_path().join("config_multiple_templates.yaml");
        let args = Args {
            config: Some(config_path),
            log_level: None,
            port: None,
            db: None,
        };

        let config = Config::from_args(args);

        assert_eq!(config.log_level, "warn");
        assert_eq!(config.port, 9000);
        assert_eq!(config.db, "multi.db");
        assert_eq!(config.templates.len(), 2);

        let first = config.templates.get("first").expect("first template should exist");
        assert_eq!(first.id_field, "id");
        assert!(first.values_yaml.is_some());

        let second = config.templates.get("second").expect("second template should exist");
        assert_eq!(second.id_field, "serial");
        assert!(second.values_yaml.is_none());
    }

    #[test]
    fn load_config_without_templates() {
        let config_path = fixtures_path().join("config_no_templates.yaml");
        let args = Args {
            config: Some(config_path),
            log_level: None,
            port: None,
            db: None,
        };

        let config = Config::from_args(args);

        assert_eq!(config.log_level, "error");
        assert_eq!(config.port, 4000);
        assert_eq!(config.db, "empty.db");
        assert!(config.templates.is_empty());
    }

    #[test]
    fn cli_args_override_config_file() {
        let config_path = fixtures_path().join("config_with_templates.yaml");
        let args = Args {
            config: Some(config_path),
            log_level: Some("trace".to_string()),
            port: Some(9999),
            db: Some("override.db".to_string()),
        };

        let config = Config::from_args(args);

        assert_eq!(config.log_level, "trace");
        assert_eq!(config.port, 9999);
        assert_eq!(config.db, "override.db");
    }

    #[test]
    fn no_config_file_uses_defaults() {
        let args = Args {
            config: None,
            log_level: None,
            port: None,
            db: None,
        };

        let config = Config::from_args(args);

        assert_eq!(config.log_level, "info");
        assert_eq!(config.port, 3000);
        assert_eq!(config.db, "provisionr.db");
        assert!(config.templates.is_empty());
        assert!(config.config_file.is_none());
    }

    #[test]
    fn resolve_path_handles_absolute_paths() {
        let config_dir = Some(PathBuf::from("/some/config/dir"));
        let absolute_path = PathBuf::from("/absolute/path/to/file.txt");

        let resolved = resolve_path(&config_dir, &absolute_path);

        assert_eq!(resolved, PathBuf::from("/absolute/path/to/file.txt"));
    }

    #[test]
    fn resolve_path_handles_relative_paths_with_config_dir() {
        let config_dir = Some(PathBuf::from("/some/config/dir"));
        let relative_path = PathBuf::from("./templates/file.txt");

        let resolved = resolve_path(&config_dir, &relative_path);

        assert_eq!(resolved, PathBuf::from("/some/config/dir/./templates/file.txt"));
    }

    #[test]
    fn resolve_path_handles_relative_paths_without_config_dir() {
        let config_dir: Option<PathBuf> = None;
        let relative_path = PathBuf::from("./templates/file.txt");

        let resolved = resolve_path(&config_dir, &relative_path);

        assert_eq!(resolved, PathBuf::from("./templates/file.txt"));
    }
}
