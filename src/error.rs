use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProvisionrError {
    #[error("Template validation failed: {0}")]
    TemplateValidation(String),

    #[error("YAML parse error: {0}")]
    YamlParse(String),

    #[error("Template render failed: {0}")]
    TemplateRender(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Template not found: {0}")]
    TemplateNotFound(String),

    #[error("Template has no content: {0}")]
    TemplateEmpty(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}
