use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::time::Duration;
use tokio::{sync::oneshot, time};
use utoipa::ToSchema;

use crate::commands::models::Command;
use crate::rest::state::AppState;

const TIMEOUT_SECS: u64 = 5;

/// Error response returned when an operation fails
#[derive(Serialize, ToSchema)]
pub struct ApiErrorResponse {
    #[schema(example = "error")]
    pub status: String,
    #[schema(example = "Template not found")]
    pub error: String,
}

impl ApiErrorResponse {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            error: msg.into(),
        }
    }
}

/// Success message returned for operations that don't return data
#[derive(Serialize, ToSchema)]
pub struct ApiSuccessMessage {
    #[schema(example = "ok")]
    pub status: String,
    #[schema(example = "Operation completed")]
    pub message: String,
}

impl ApiSuccessMessage {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            status: "ok".to_string(),
            message: msg.into(),
        }
    }
}

pub enum CommandError {
    Timeout,
    ChannelClosed,
    Handler(String),
    HandlerUnavailable,
}

impl CommandError {
    pub fn into_plain_response(self) -> Response {
        let (status, message) = match &self {
            Self::Timeout => (StatusCode::GATEWAY_TIMEOUT, "Request timeout"),
            Self::ChannelClosed => (StatusCode::INTERNAL_SERVER_ERROR, "Channel closed"),
            Self::Handler(e) => (StatusCode::BAD_REQUEST, e.as_str()),
            Self::HandlerUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "Handler unavailable"),
        };
        (status, message.to_string()).into_response()
    }
}

impl IntoResponse for CommandError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            Self::Timeout => (StatusCode::GATEWAY_TIMEOUT, "timeout"),
            Self::ChannelClosed => (StatusCode::INTERNAL_SERVER_ERROR, "channel closed"),
            Self::Handler(e) => (StatusCode::BAD_REQUEST, e.as_str()),
            Self::HandlerUnavailable => (StatusCode::SERVICE_UNAVAILABLE, "handler-unavailable"),
        };
        (status, Json(ApiErrorResponse::new(message))).into_response()
    }
}

pub async fn await_response<T>(rx: oneshot::Receiver<Result<T, String>>) -> Result<T, CommandError> {
    match time::timeout(Duration::from_secs(TIMEOUT_SECS), rx).await {
        Ok(Ok(Ok(value))) => Ok(value),
        Ok(Ok(Err(e))) => Err(CommandError::Handler(e)),
        Ok(Err(_)) => Err(CommandError::ChannelClosed),
        Err(_) => Err(CommandError::Timeout),
    }
}

pub async fn send_command<T>(
    state: &AppState,
    cmd_fn: impl FnOnce(oneshot::Sender<Result<T, String>>) -> Command,
) -> Result<T, CommandError> {
    let (tx, rx) = oneshot::channel();
    state
        .command_tx
        .send(cmd_fn(tx))
        .await
        .map_err(|_| CommandError::HandlerUnavailable)?;
    await_response(rx).await
}
