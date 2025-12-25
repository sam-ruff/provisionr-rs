use crate::commands::models::Command;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct AppState {
    pub command_tx: mpsc::Sender<Command>,
}
