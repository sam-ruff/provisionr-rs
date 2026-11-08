use minijinja::{context, Environment, Value};
use std::collections::HashMap;

#[cfg_attr(test, mockall::automock)]
pub trait TemplateEngine: Send {
    fn validate(&self, template_content: &str) -> Result<(), String>;
    fn render(
        &self,
        template_content: &str,
        values: &HashMap<String, String>,
    ) -> Result<String, String>;
}

pub struct MiniJinjaEngine;

impl MiniJinjaEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MiniJinjaEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateEngine for MiniJinjaEngine {
    fn validate(&self, template_content: &str) -> Result<(), String> {
        let mut env = Environment::new();
        env.add_template("template", template_content)
            .map_err(|e| format!("Template validation error: {}", e))?;
        Ok(())
    }

    fn render(
        &self,
        template_content: &str,
        values: &HashMap<String, String>,
    ) -> Result<String, String> {
        let mut env = Environment::new();
        env.add_template("template", template_content)
            .map_err(|e| format!("Template parse error: {}", e))?;

        let template = env
            .get_template("template")
            .map_err(|e| format!("Template retrieval error: {}", e))?;

        let ctx: HashMap<&str, Value> = values
            .iter()
            .map(|(k, v)| (k.as_str(), Value::from(v.clone())))
            .collect();

        template
            .render(context!(..ctx))
            .map_err(|e| format!("Template render error: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quickcheck_macros::quickcheck;

    #[test]
    fn validate_valid_template() {
        let engine = MiniJinjaEngine::new();
        assert!(engine.validate("Hello, {{ name }}!").is_ok());
    }

    #[test]
    fn validate_invalid_template() {
        let engine = MiniJinjaEngine::new();
        assert!(engine.validate("Hello, {{ name }").is_err());
    }

    #[quickcheck]
    fn render_substitutes_value_correctly(value: String) -> bool {
        let engine = MiniJinjaEngine::new();
        let mut values = HashMap::new();
        values.insert("name".to_string(), value.clone());

        let result = engine.render("{{ name }}", &values);
        result.map(|r| r == value).unwrap_or(false)
    }

    #[quickcheck]
    fn render_with_multiple_values_contains_all(a: String, b: String) -> bool {
        let engine = MiniJinjaEngine::new();
        let mut values = HashMap::new();
        values.insert("a".to_string(), a.clone());
        values.insert("b".to_string(), b.clone());

        let result = engine.render("{{ a }}|{{ b }}", &values);
        result
            .map(|r| r == format!("{}|{}", a, b))
            .unwrap_or(false)
    }

    #[test]
    fn render_with_conditionals() {
        let engine = MiniJinjaEngine::new();
        let mut values = HashMap::new();
        values.insert("enable_feature".to_string(), "yes".to_string());

        let template =
            r#"{% if enable_feature == "yes" %}Feature enabled{% else %}Feature disabled{% endif %}"#;
        let result = engine.render(template, &values);
        assert_eq!(result.unwrap(), "Feature enabled");
    }

    #[test]
    fn render_undefined_variable_is_empty() {
        let engine = MiniJinjaEngine::new();
        let result = engine.render("[{{ missing }}]", &HashMap::new());
        assert_eq!(result.unwrap(), "[]");
    }

    #[test]
    fn render_does_not_html_escape_values() {
        let engine = MiniJinjaEngine::new();
        let mut values = HashMap::new();
        values.insert("cmd".to_string(), r#"echo "<a & b>" > 'out'"#.to_string());

        let result = engine.render("{{ cmd }}", &values);
        assert_eq!(result.unwrap(), r#"echo "<a & b>" > 'out'"#);
    }

    #[test]
    fn render_reports_runtime_errors() {
        let engine = MiniJinjaEngine::new();
        let result = engine.render("{{ 1 // 0 }}", &HashMap::new());
        assert!(result.unwrap_err().starts_with("Template render error"));
    }
}
