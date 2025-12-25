use crate::storage::models::{DynamicFieldConfig, TemplateData};
use evmap::handles::{ReadHandle, WriteHandle};

#[cfg_attr(test, mockall::automock)]
pub trait TemplateStore: Send {
    fn set_template_content(&mut self, name: &str, content: String);
    fn set_values(&mut self, name: &str, yaml_str: String) -> Result<(), String>;
    fn set_id_field(&mut self, name: &str, id_field: String) -> Result<(), String>;
    fn set_dynamic_fields(&mut self, name: &str, fields: Vec<DynamicFieldConfig>) -> Result<(), String>;
    fn get(&self, name: &str) -> Option<TemplateData>;
    fn delete(&mut self, name: &str);
    fn exists(&self, name: &str) -> bool;
}

pub struct EvmapTemplateStore {
    read: ReadHandle<String, TemplateData>,
    write: WriteHandle<String, TemplateData>,
}

impl EvmapTemplateStore {
    pub fn new() -> Self {
        // SAFETY: TemplateData contains only String and Vec<DynamicFieldConfig>
        // which have stable Hash and Eq implementations.
        let (write, read) = unsafe { evmap::new_assert_stable() };
        Self { read, write }
    }

    fn get_or_default(&self, name: &str) -> TemplateData {
        self.read
            .get_one(name)
            .map(|guard| (*guard).clone())
            .unwrap_or_default()
    }

    fn update(&mut self, name: &str, data: TemplateData) {
        self.write.clear(name.to_string());
        self.write.insert(name.to_string(), data);
    }

    fn update_template(&mut self, name: &str, update_fn: impl FnOnce(&mut TemplateData)) {
        let mut data = self.get_or_default(name);
        update_fn(&mut data);
        self.update(name, data);
        self.write.publish();
    }
}

impl Default for EvmapTemplateStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateStore for EvmapTemplateStore {
    fn set_template_content(&mut self, name: &str, content: String) {
        self.update_template(name, |d| d.template_content = content);
    }

    fn set_values(&mut self, name: &str, yaml_str: String) -> Result<(), String> {
        if !self.exists(name) {
            return Err(format!("Template '{}' not found", name));
        }
        self.update_template(name, |d| d.values_yaml = Some(yaml_str));
        Ok(())
    }

    fn set_id_field(&mut self, name: &str, id_field: String) -> Result<(), String> {
        if !self.exists(name) {
            return Err(format!("Template '{}' not found", name));
        }
        self.update_template(name, |d| d.id_field = id_field);
        Ok(())
    }

    fn set_dynamic_fields(&mut self, name: &str, fields: Vec<DynamicFieldConfig>) -> Result<(), String> {
        if !self.exists(name) {
            return Err(format!("Template '{}' not found", name));
        }
        self.update_template(name, |d| d.dynamic_fields = fields);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<TemplateData> {
        self.read.get_one(name).map(|guard| (*guard).clone())
    }

    fn delete(&mut self, name: &str) {
        self.write.clear(name.to_string());
        self.write.publish();
    }

    fn exists(&self, name: &str) -> bool {
        self.read.get_one(name).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::GeneratorType;

    #[test]
    fn set_template_content_is_immediately_readable() {
        let mut store = EvmapTemplateStore::new();

        assert!(store.get("test.j2").is_none());

        store.set_template_content("test.j2", "Hello {{ name }}".to_string());

        let data = store.get("test.j2").expect("Should be readable immediately after set");
        assert_eq!(data.template_content, "Hello {{ name }}");
    }

    #[test]
    fn set_values_is_immediately_readable() {
        let mut store = EvmapTemplateStore::new();

        store.set_template_content("test.j2", "content".to_string());
        store.set_values("test.j2", "key: value".to_string()).unwrap();

        let data = store.get("test.j2").unwrap();
        assert_eq!(data.values_yaml, Some("key: value".to_string()));
    }

    #[test]
    fn set_values_fails_if_template_not_found() {
        let mut store = EvmapTemplateStore::new();

        let result = store.set_values("nonexistent.j2", "key: value".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn set_id_field_is_immediately_readable() {
        let mut store = EvmapTemplateStore::new();

        store.set_template_content("test.j2", "content".to_string());
        store.set_id_field("test.j2", "serial_number".to_string()).unwrap();

        let data = store.get("test.j2").unwrap();
        assert_eq!(data.id_field, "serial_number");
    }

    #[test]
    fn set_id_field_fails_if_template_not_found() {
        let mut store = EvmapTemplateStore::new();

        let result = store.set_id_field("nonexistent.j2", "serial".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn set_dynamic_fields_is_immediately_readable() {
        let mut store = EvmapTemplateStore::new();

        store.set_template_content("test.j2", "content".to_string());
        store.set_dynamic_fields(
            "test.j2",
            vec![DynamicFieldConfig {
                field_name: "password".to_string(),
                generator_type: GeneratorType::Alphanumeric(16),
            }],
        ).unwrap();

        let data = store.get("test.j2").unwrap();
        assert_eq!(data.dynamic_fields.len(), 1);
        assert_eq!(data.dynamic_fields[0].field_name, "password");
    }

    #[test]
    fn set_dynamic_fields_fails_if_template_not_found() {
        let mut store = EvmapTemplateStore::new();

        let result = store.set_dynamic_fields(
            "nonexistent.j2",
            vec![DynamicFieldConfig {
                field_name: "password".to_string(),
                generator_type: GeneratorType::Alphanumeric(16),
            }],
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn delete_is_immediately_effective() {
        let mut store = EvmapTemplateStore::new();

        store.set_template_content("test.j2", "content".to_string());
        assert!(store.get("test.j2").is_some());

        store.delete("test.j2");
        assert!(store.get("test.j2").is_none());
    }

    #[test]
    fn multiple_updates_are_all_visible() {
        let mut store = EvmapTemplateStore::new();

        store.set_template_content("test.j2", "Hello".to_string());
        store.set_values("test.j2", "name: World".to_string()).unwrap();
        store.set_id_field("test.j2", "mac".to_string()).unwrap();

        let data = store.get("test.j2").unwrap();
        assert_eq!(data.template_content, "Hello");
        assert_eq!(data.values_yaml, Some("name: World".to_string()));
        assert_eq!(data.id_field, "mac");
    }

    #[test]
    fn exists_returns_true_for_existing_template() {
        let mut store = EvmapTemplateStore::new();

        store.set_template_content("test.j2", "content".to_string());
        assert!(store.exists("test.j2"));
    }

    #[test]
    fn exists_returns_false_for_nonexistent_template() {
        let store = EvmapTemplateStore::new();

        assert!(!store.exists("nonexistent.j2"));
    }
}
