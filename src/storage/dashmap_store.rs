use dashmap::DashMap;

use crate::storage::models::{TemplateConfig, TemplateData};

#[cfg_attr(test, mockall::automock)]
pub trait TemplateStore: Send {
    fn set_template_content(&mut self, name: &str, content: String);
    fn set_values(&mut self, name: &str, yaml_str: String) -> Result<(), String>;
    fn set_config(&mut self, name: &str, config: TemplateConfig) -> Result<(), String>;
    fn get_config(&self, name: &str) -> Option<TemplateConfig>;
    fn get(&self, name: &str) -> Option<TemplateData>;
    fn delete(&mut self, name: &str);
}

pub struct DashMapTemplateStore {
    map: DashMap<String, TemplateData>,
}

impl DashMapTemplateStore {
    pub fn new() -> Self {
        Self { map: DashMap::new() }
    }
}

impl Default for DashMapTemplateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateStore for DashMapTemplateStore {
    fn set_template_content(&mut self, name: &str, content: String) {
        self.map
            .entry(name.to_string())
            .or_default()
            .template_content = content;
    }

    fn set_values(&mut self, name: &str, yaml_str: String) -> Result<(), String> {
        match self.map.get_mut(name) {
            Some(mut entry) => {
                entry.values_yaml = Some(yaml_str);
                Ok(())
            }
            None => Err(format!("Template '{}' not found", name)),
        }
    }

    fn set_config(&mut self, name: &str, config: TemplateConfig) -> Result<(), String> {
        match self.map.get_mut(name) {
            Some(mut entry) => {
                entry.id_field = config.id_field;
                entry.dynamic_fields = config.dynamic_fields;
                entry.hashing_algorithm = config.hashing_algorithm;
                Ok(())
            }
            None => Err(format!("Template '{}' not found", name)),
        }
    }

    fn get_config(&self, name: &str) -> Option<TemplateConfig> {
        self.map.get(name).map(|data| TemplateConfig {
            id_field: data.id_field.clone(),
            dynamic_fields: data.dynamic_fields.clone(),
            hashing_algorithm: data.hashing_algorithm.clone(),
        })
    }

    fn get(&self, name: &str) -> Option<TemplateData> {
        self.map.get(name).map(|r| r.clone())
    }

    fn delete(&mut self, name: &str) {
        self.map.remove(name);
    }
}

#[cfg(test)]
impl DashMapTemplateStore {
    pub fn exists(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{DynamicFieldConfig, GeneratorType, HashingAlgorithm};

    #[test]
    fn set_template_content_is_immediately_readable() {
        let mut store = DashMapTemplateStore::new();

        assert!(store.get("test").is_none());

        store.set_template_content("test", "Hello {{ name }}".to_string());

        let data = store.get("test").expect("Should be readable immediately after set");
        assert_eq!(data.template_content, "Hello {{ name }}");
    }

    #[test]
    fn set_values_is_immediately_readable() {
        let mut store = DashMapTemplateStore::new();

        store.set_template_content("test", "content".to_string());
        store.set_values("test", "key: value".to_string()).unwrap();

        let data = store.get("test").unwrap();
        assert_eq!(data.values_yaml, Some("key: value".to_string()));
    }

    #[test]
    fn set_values_fails_if_template_not_found() {
        let mut store = DashMapTemplateStore::new();

        let result = store.set_values("nonexistent", "key: value".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn set_config_is_immediately_readable() {
        let mut store = DashMapTemplateStore::new();

        store.set_template_content("test", "content".to_string());
        store
            .set_config(
                "test",
                TemplateConfig {
                    id_field: "serial_number".to_string(),
                    dynamic_fields: vec![DynamicFieldConfig {
                        field_name: "password".to_string(),
                        generator_type: GeneratorType::Alphanumeric { length: 16 },
                    }],
                    hashing_algorithm: HashingAlgorithm::Sha512,
                },
            )
            .unwrap();

        let data = store.get("test").unwrap();
        assert_eq!(data.id_field, "serial_number");
        assert_eq!(data.dynamic_fields.len(), 1);
        assert_eq!(data.dynamic_fields[0].field_name, "password");
        assert_eq!(data.hashing_algorithm, HashingAlgorithm::Sha512);
    }

    #[test]
    fn set_config_fails_if_template_not_found() {
        let mut store = DashMapTemplateStore::new();

        let result = store.set_config(
            "nonexistent",
            TemplateConfig {
                id_field: "serial".to_string(),
                dynamic_fields: vec![],
                hashing_algorithm: HashingAlgorithm::None,
            },
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn get_config_returns_template_config() {
        let mut store = DashMapTemplateStore::new();

        store.set_template_content("test", "content".to_string());
        store
            .set_config(
                "test",
                TemplateConfig {
                    id_field: "mac".to_string(),
                    dynamic_fields: vec![DynamicFieldConfig {
                        field_name: "pass".to_string(),
                        generator_type: GeneratorType::Passphrase { word_count: 4 },
                    }],
                    hashing_algorithm: HashingAlgorithm::Yescrypt,
                },
            )
            .unwrap();

        let config = store.get_config("test").unwrap();
        assert_eq!(config.id_field, "mac");
        assert_eq!(config.dynamic_fields.len(), 1);
        assert_eq!(config.hashing_algorithm, HashingAlgorithm::Yescrypt);
    }

    #[test]
    fn get_config_returns_none_for_nonexistent() {
        let store = DashMapTemplateStore::new();

        assert!(store.get_config("nonexistent").is_none());
    }

    #[test]
    fn delete_is_immediately_effective() {
        let mut store = DashMapTemplateStore::new();

        store.set_template_content("test", "content".to_string());
        assert!(store.get("test").is_some());

        store.delete("test");
        assert!(store.get("test").is_none());
    }

    #[test]
    fn multiple_updates_are_all_visible() {
        let mut store = DashMapTemplateStore::new();

        store.set_template_content("test", "Hello".to_string());
        store.set_values("test", "name: World".to_string()).unwrap();
        store
            .set_config(
                "test",
                TemplateConfig {
                    id_field: "mac".to_string(),
                    dynamic_fields: vec![],
                    hashing_algorithm: HashingAlgorithm::None,
                },
            )
            .unwrap();

        let data = store.get("test").unwrap();
        assert_eq!(data.template_content, "Hello");
        assert_eq!(data.values_yaml, Some("name: World".to_string()));
        assert_eq!(data.id_field, "mac");
    }

    #[test]
    fn exists_returns_true_for_existing_template() {
        let mut store = DashMapTemplateStore::new();

        store.set_template_content("test", "content".to_string());
        assert!(store.exists("test"));
    }

    #[test]
    fn exists_returns_false_for_nonexistent_template() {
        let store = DashMapTemplateStore::new();

        assert!(!store.exists("nonexistent"));
    }
}
