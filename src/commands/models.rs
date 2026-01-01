use std::collections::HashMap;
use tokio::sync::oneshot;

use crate::storage::models::{RenderedTemplate, RenderedTemplateSummary, TemplateConfig};

pub enum Command {
    SetTemplate {
        name: String,
        content: String,
        response: oneshot::Sender<Result<(), String>>,
    },
    SetValues {
        name: String,
        yaml: String,
        response: oneshot::Sender<Result<(), String>>,
    },
    SetConfig {
        name: String,
        config: TemplateConfig,
        response: oneshot::Sender<Result<(), String>>,
    },
    GetConfig {
        name: String,
        response: oneshot::Sender<Result<Option<TemplateConfig>, String>>,
    },
    RenderTemplate {
        name: String,
        query_values: HashMap<String, String>,
        response: oneshot::Sender<Result<String, String>>,
    },
    ListRendered {
        template_name: String,
        response: oneshot::Sender<Result<Vec<RenderedTemplateSummary>, String>>,
    },
    GetRendered {
        template_name: String,
        id_value: String,
        response: oneshot::Sender<Result<Option<RenderedTemplate>, String>>,
    },
    DeleteTemplate {
        name: String,
        response: oneshot::Sender<Result<(), String>>,
    },
}
