use std::collections::HashMap;
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

use crate::error::ProvisionrError;
use crate::generators::{AlphanumericGenerator, PassphraseGenerator, ValueGenerator};
use crate::storage::models::{DynamicFieldConfig, GeneratorType};
use crate::templating::TemplateEngine;

#[cfg_attr(test, mockall::automock)]
pub trait Commander: Send {
    fn validate_template(&self, template_content: &str) -> Result<(), ProvisionrError>;
    fn render_template(
        &self,
        template_content: &str,
        values: &HashMap<String, String>,
    ) -> Result<String, ProvisionrError>;
    fn generate_dynamic_values(&self, fields: &[DynamicFieldConfig]) -> HashMap<String, String>;
    fn parse_yaml(&self, yaml_str: &str) -> Result<Yaml, ProvisionrError>;
    fn yaml_to_map(&self, yaml: &Yaml) -> HashMap<String, String>;
    fn map_to_yaml_string(&self, map: &HashMap<String, String>) -> Result<String, ProvisionrError>;
}

pub struct ConcreteCommander<E: TemplateEngine> {
    engine: E,
}

impl<E: TemplateEngine> ConcreteCommander<E> {
    pub fn new(engine: E) -> Self {
        Self { engine }
    }
}

impl<E: TemplateEngine + Send> Commander for ConcreteCommander<E> {
    fn validate_template(&self, template_content: &str) -> Result<(), ProvisionrError> {
        self.engine
            .validate(template_content)
            .map_err(ProvisionrError::TemplateValidation)
    }

    fn render_template(
        &self,
        template_content: &str,
        values: &HashMap<String, String>,
    ) -> Result<String, ProvisionrError> {
        self.engine
            .render(template_content, values)
            .map_err(ProvisionrError::TemplateRender)
    }

    fn generate_dynamic_values(&self, fields: &[DynamicFieldConfig]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for field in fields {
            let generator: Box<dyn ValueGenerator> = match &field.generator_type {
                GeneratorType::Alphanumeric(len) => Box::new(AlphanumericGenerator::new(*len)),
                GeneratorType::Passphrase(count) => Box::new(PassphraseGenerator::new(*count)),
            };
            result.insert(field.field_name.clone(), generator.generate());
        }
        result
    }

    fn parse_yaml(&self, yaml_str: &str) -> Result<Yaml, ProvisionrError> {
        let docs = YamlLoader::load_from_str(yaml_str)
            .map_err(|e| ProvisionrError::YamlParse(e.to_string()))?;
        docs.into_iter()
            .next()
            .ok_or_else(|| ProvisionrError::YamlParse("Empty YAML document".to_string()))
    }

    fn yaml_to_map(&self, yaml: &Yaml) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Yaml::Hash(hash) = yaml {
            for (key, value) in hash {
                if let Yaml::String(k) = key {
                    let v = match value {
                        Yaml::String(s) => s.clone(),
                        Yaml::Integer(i) => i.to_string(),
                        Yaml::Real(r) => r.clone(),
                        Yaml::Boolean(b) => b.to_string(),
                        _ => continue,
                    };
                    map.insert(k.clone(), v);
                }
            }
        }
        map
    }

    fn map_to_yaml_string(&self, map: &HashMap<String, String>) -> Result<String, ProvisionrError> {
        let mut yaml_hash = yaml_rust2::yaml::Hash::new();
        for (k, v) in map {
            yaml_hash.insert(Yaml::String(k.clone()), Yaml::String(v.clone()));
        }
        let yaml = Yaml::Hash(yaml_hash);

        let mut out_str = String::new();
        let mut emitter = YamlEmitter::new(&mut out_str);
        emitter
            .dump(&yaml)
            .map_err(|e| ProvisionrError::YamlParse(format!("YAML emit error: {}", e)))?;

        Ok(out_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templating::{MiniJinjaEngine, MockTemplateEngine};
    use mockall::predicate::*;
    use quickcheck_macros::quickcheck;

    fn create_commander() -> ConcreteCommander<MiniJinjaEngine> {
        ConcreteCommander::new(MiniJinjaEngine::new())
    }

    mod mock_tests {
        use super::*;

        #[test]
        fn validate_template_delegates_to_engine() {
            let mut mock_engine = MockTemplateEngine::new();
            mock_engine
                .expect_validate()
                .with(eq("{{ name }}"))
                .times(1)
                .returning(|_| Ok(()));

            let commander = ConcreteCommander::new(mock_engine);
            assert!(commander.validate_template("{{ name }}").is_ok());
        }

        #[test]
        fn validate_template_propagates_engine_error() {
            let mut mock_engine = MockTemplateEngine::new();
            mock_engine
                .expect_validate()
                .times(1)
                .returning(|_| Err("Invalid syntax".to_string()));

            let commander = ConcreteCommander::new(mock_engine);
            let result = commander.validate_template("{{ bad");
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Invalid syntax"));
        }

        #[test]
        fn render_template_delegates_to_engine() {
            let mut mock_engine = MockTemplateEngine::new();
            mock_engine
                .expect_render()
                .withf(|template, values| {
                    template == "Hello {{ name }}"
                        && values.get("name") == Some(&"World".to_string())
                })
                .times(1)
                .returning(|_, _| Ok("Hello World".to_string()));

            let commander = ConcreteCommander::new(mock_engine);
            let mut values = HashMap::new();
            values.insert("name".to_string(), "World".to_string());

            let result = commander.render_template("Hello {{ name }}", &values);
            assert_eq!(result.unwrap(), "Hello World");
        }

        #[test]
        fn render_template_propagates_engine_error() {
            let mut mock_engine = MockTemplateEngine::new();
            mock_engine
                .expect_render()
                .times(1)
                .returning(|_, _| Err("Missing variable".to_string()));

            let commander = ConcreteCommander::new(mock_engine);
            let values = HashMap::new();

            let result = commander.render_template("{{ undefined }}", &values);
            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("Missing variable"));
        }
    }

    #[test]
    fn validate_template() {
        let commander = create_commander();
        assert!(commander.validate_template("Hello {{ name }}").is_ok());
        assert!(commander.validate_template("Hello {{ name }").is_err());
    }

    #[quickcheck]
    fn render_template_substitutes_value(value: String) -> bool {
        let commander = create_commander();
        let mut values = HashMap::new();
        values.insert("name".to_string(), value.clone());

        commander
            .render_template("{{ name }}", &values)
            .map(|r| r == value)
            .unwrap_or(false)
    }

    #[quickcheck]
    fn generate_alphanumeric_correct_length(len: u8) -> bool {
        let len = (len as usize).max(1).min(100);
        let commander = create_commander();
        let fields = vec![DynamicFieldConfig {
            field_name: "password".to_string(),
            generator_type: GeneratorType::Alphanumeric(len),
        }];

        let result = commander.generate_dynamic_values(&fields);
        result
            .get("password")
            .map(|p| p.len() == len)
            .unwrap_or(false)
    }

    #[quickcheck]
    fn generate_passphrase_correct_word_count(count: u8) -> bool {
        // Use % 9 + 1 to guarantee range 1-9
        let count = (count as usize % 9) + 1;
        let commander = create_commander();
        let fields = vec![DynamicFieldConfig {
            field_name: "passphrase".to_string(),
            generator_type: GeneratorType::Passphrase(count),
        }];

        let result = commander.generate_dynamic_values(&fields);
        result
            .get("passphrase")
            .map(|p| p.split('-').count() == count)
            .unwrap_or(false)
    }

    #[quickcheck]
    fn yaml_roundtrip_preserves_simple_values(key: String, value: String) -> bool {
        // Filter out problematic YAML characters and control characters
        let has_control_chars = |s: &str| s.chars().any(|c| c.is_control());
        // YAML strips leading/trailing whitespace from keys, so filter those out
        let has_problematic_whitespace = |s: &str| s.trim().is_empty() || s != s.trim();
        if key.is_empty()
            || key.contains(':')
            || key.contains('#')
            || key.contains('[')
            || key.contains(']')
            || key.contains('{')
            || key.contains('}')
            || has_control_chars(&key)
            || has_problematic_whitespace(&key)
        {
            return true;
        }
        if has_control_chars(&value) || has_problematic_whitespace(&value) {
            return true;
        }

        let commander = create_commander();
        let yaml_str = format!("{}: {}", key, value);

        commander
            .parse_yaml(&yaml_str)
            .map(|yaml| {
                let map = commander.yaml_to_map(&yaml);
                map.get(&key).map(|v| v == &value).unwrap_or(false)
            })
            .unwrap_or(false)
    }

    #[test]
    fn parse_yaml_with_multiple_types() {
        let commander = create_commander();
        let yaml_str = "name: test\nvalue: 123\nflag: true";
        let yaml = commander.parse_yaml(yaml_str).unwrap();
        let map = commander.yaml_to_map(&yaml);

        assert_eq!(map.get("name"), Some(&"test".to_string()));
        assert_eq!(map.get("value"), Some(&"123".to_string()));
        assert_eq!(map.get("flag"), Some(&"true".to_string()));
    }
}
