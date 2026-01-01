use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Generator type with tagged serialisation
/// Serialises to: {"type": "alphanumeric", "length": 16}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GeneratorType {
    Alphanumeric {
        #[schema(example = 32)]
        length: usize,
    },
    Passphrase {
        #[schema(example = 4)]
        word_count: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema)]
pub struct DynamicFieldConfig {
    #[schema(example = "luks_password")]
    pub field_name: String,
    #[serde(flatten)]
    pub generator_type: GeneratorType,
    /// Algorithm used to hash the generated value. Use 'sha512' or 'yescrypt' for
    /// password fields that require crypt-format hashes.
    #[serde(default)]
    #[schema(example = "sha512")]
    pub hashing_algorithm: HashingAlgorithm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, ToSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum HashingAlgorithm {
    #[default]
    None,
    Sha512,
    Yescrypt,
}

fn default_id_field() -> String {
    "mac_address".to_string()
}

/// Configuration for template rendering behaviour including caching and dynamic value generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema, Default)]
pub struct TemplateConfig {
    /// Field name used to uniquely identify render requests. Renders with the same id_field
    /// value return cached results. For kickstart provisioning, use 'mac_address' to ensure
    /// the same machine receives consistent templates across multiple boot attempts.
    #[serde(default = "default_id_field")]
    #[schema(example = "mac_address")]
    pub id_field: String,
    /// Fields whose values are generated at render time rather than provided statically.
    /// Commonly used for passwords that need to be generated and optionally hashed, such as
    /// LUKS encryption passwords in kickstart templates. Each field can specify its own
    /// hashing algorithm.
    #[serde(default)]
    pub dynamic_fields: Vec<DynamicFieldConfig>,
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
