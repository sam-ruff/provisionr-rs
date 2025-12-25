use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
pub enum GeneratorType {
    Alphanumeric(usize),
    Passphrase(usize),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
pub struct DynamicFieldConfig {
    pub field_name: String,
    pub generator_type: GeneratorType,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, ToSchema)]
pub struct TemplateData {
    pub template_content: String,
    pub id_field: String,
    pub values_yaml: Option<String>,
    pub dynamic_fields: Vec<DynamicFieldConfig>,
}

impl Default for TemplateData {
    fn default() -> Self {
        Self {
            template_content: String::new(),
            id_field: "mac_address".to_string(),
            values_yaml: None,
            dynamic_fields: Vec::new(),
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RenderedTemplate {
    pub id: i64,
    pub template_name: String,
    pub id_field_value: String,
    pub rendered_content: String,
    pub generated_values: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RenderedTemplateSummary {
    pub id_field_value: String,
    pub created_at: String,
}
