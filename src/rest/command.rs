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

#[derive(Serialize, ToSchema)]
pub struct ApiResponse<T: Serialize> {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            status: "ok".to_string(),
            data: Some(data),
            error: None,
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: "not_found".to_string(),
            data: None,
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            error: Some(msg.into()),
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
        (status, Json(ApiResponse::<()>::error(message))).into_response()
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
