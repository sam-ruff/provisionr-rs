use crate::commands::commander::Commander;
use crate::commands::models::Command;
use crate::error::ProvisionrError;
use crate::statics::shutdown::global_cancellation_token;
use crate::storage::{RenderedStore, TemplateStore};
use async_trait::async_trait;
use log::{debug, info};
use std::collections::HashMap;
use tokio::sync::mpsc::Receiver;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait Handler<C: Commander, T: TemplateStore, R: RenderedStore>: Send {
    fn new(commander: C, template_store: T, rendered_store: R, rx: Receiver<Command>) -> Self;
    async fn main_loop(&mut self);
}

pub struct ConcreteHandler<C: Commander + Send, T: TemplateStore, R: RenderedStore> {
    commander: C,
    template_store: T,
    rendered_store: R,
    rx: Receiver<Command>,
    cancel_token: CancellationToken,
}

#[async_trait]
impl<C, T, R> Handler<C, T, R> for ConcreteHandler<C, T, R>
where
    C: Commander + Send,
    T: TemplateStore,
    R: RenderedStore,
{
    fn new(commander: C, template_store: T, rendered_store: R, rx: Receiver<Command>) -> Self {
        Self {
            commander,
            template_store,
            rendered_store,
            rx,
            cancel_token: global_cancellation_token(),
        }
    }

    async fn main_loop(&mut self) {
        loop {
            tokio::select! {
                _ = self.cancel_token.cancelled() => {
                    debug!("Handler thread cancelled. Shutting down.");
                    break;
                }

                cmd_option = self.rx.recv() => {
                    match cmd_option {
                        Some(cmd) => self.handle_command(cmd),
                        None => break,
                    }
                }
            }
        }
    }
}

impl<C, T, R> ConcreteHandler<C, T, R>
where
    C: Commander + Send,
    T: TemplateStore,
    R: RenderedStore,
{
    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::SetTemplate {
                name,
                content,
                response,
            } => {
                let result = self.handle_set_template(&name, content).map_err(|e| e.to_string());
                let _ = response.send(result);
            }

            Command::SetValues {
                name,
                yaml,
                response,
            } => {
                let result = self.handle_set_values(&name, &yaml).map_err(|e| e.to_string());
                let _ = response.send(result);
            }

            Command::SetConfig {
                name,
                config,
                response,
            } => {
                let result = self.template_store.set_config(&name, config);
                let _ = response.send(result);
            }

            Command::GetConfig { name, response } => {
                let result = Ok(self.template_store.get_config(&name));
                let _ = response.send(result);
            }

            Command::RenderTemplate {
                name,
                query_values,
                response,
            } => {
                let result = self.handle_render(&name, query_values).map_err(|e| e.to_string());
                let _ = response.send(result);
            }

            Command::ListRendered {
                template_name,
                response,
            } => {
                let result = self.rendered_store.list_rendered(&template_name).map_err(|e| e.to_string());
                let _ = response.send(result);
            }

            Command::GetRendered {
                template_name,
                id_value,
                response,
            } => {
                let result = self.rendered_store.get_rendered(&template_name, &id_value).map_err(|e| e.to_string());
                let _ = response.send(result);
            }

            Command::DeleteTemplate { name, response } => {
                self.template_store.delete(&name);
                info!("Template '{}' deleted", name);
                let _ = response.send(Ok(()));
            }
        }
    }

    fn handle_set_template(&mut self, name: &str, content: String) -> Result<(), ProvisionrError> {
        self.commander.validate_template(&content)?;

        self.template_store.set_template_content(name, content);
        info!("Template '{}' set successfully", name);
        Ok(())
    }

    fn handle_set_values(&mut self, name: &str, yaml_str: &str) -> Result<(), ProvisionrError> {
        self.commander.parse_yaml(yaml_str)?;
        self.template_store
            .set_values(name, yaml_str.to_string())
            .map_err(ProvisionrError::TemplateNotFound)?;
        info!("Values for template '{}' set successfully", name);
        Ok(())
    }

    fn handle_render(
        &mut self,
        name: &str,
        query_values: HashMap<String, String>,
    ) -> Result<String, ProvisionrError> {
        let template_data = self
            .template_store
            .get(name)
            .ok_or_else(|| ProvisionrError::TemplateNotFound(name.to_string()))?;

        if template_data.template_content.is_empty() {
            return Err(ProvisionrError::TemplateEmpty(name.to_string()));
        }

        let id_value = query_values
            .get(&template_data.id_field)
            .ok_or_else(|| ProvisionrError::MissingField(template_data.id_field.clone()))?;

        if let Ok(Some(cached)) = self.rendered_store.get_rendered(name, id_value) {
            info!("Returning cached render for {}:{}", name, id_value);
            return Ok(cached.rendered_content);
        }

        let mut values = if let Some(yaml_str) = &template_data.values_yaml {
            let yaml = self.commander.parse_yaml(yaml_str)?;
            self.commander.yaml_to_map(&yaml)
        } else {
            HashMap::new()
        };

        for (k, v) in &query_values {
            values.insert(k.clone(), v.clone());
        }

        let generated = self
            .commander
            .generate_dynamic_values(&template_data.dynamic_fields, &template_data.hashing_algorithm);
        let generated_yaml = self.commander.map_to_yaml_string(&generated)?;

        for (k, v) in &generated {
            values.insert(k.clone(), v.clone());
        }

        let rendered = self
            .commander
            .render_template(&template_data.template_content, &values)?;

        self.rendered_store
            .store_rendered(name, id_value, &rendered, &generated_yaml)?;

        info!("Rendered and stored template for {}:{}", name, id_value);
        Ok(rendered)
    }

    #[cfg(test)]
    pub fn new_with_token(
        commander: C,
        template_store: T,
        rendered_store: R,
        rx: Receiver<Command>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            commander,
            template_store,
            rendered_store,
            rx,
            cancel_token,
        }
    }

    #[cfg(test)]
    pub fn process_command(&mut self, cmd: Command) {
        self.handle_command(cmd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::MockCommander;
    use crate::storage::models::{
        DynamicFieldConfig, GeneratorType, HashingAlgorithm, RenderedTemplate, TemplateConfig,
        TemplateData,
    };
    use crate::storage::{MockRenderedStore, MockTemplateStore};
    use mockall::predicate::*;
    use tokio::sync::{mpsc, oneshot};
    use yaml_rust2::YamlLoader;

    fn create_test_handler(
        commander: MockCommander,
        template_store: MockTemplateStore,
        rendered_store: MockRenderedStore,
    ) -> ConcreteHandler<MockCommander, MockTemplateStore, MockRenderedStore> {
        let (_tx, rx) = mpsc::channel(1);
        let cancel_token = CancellationToken::new();
        ConcreteHandler::new_with_token(commander, template_store, rendered_store, rx, cancel_token)
    }

    #[test]
    fn set_template_validates_template_content() {
        let mut commander = MockCommander::new();
        commander
            .expect_validate_template()
            .with(eq("{{ invalid"))
            .times(1)
            .returning(|_| Err(ProvisionrError::TemplateValidation("Syntax error".to_string())));

        let template_store = MockTemplateStore::new();
        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::SetTemplate {
            name: "template".to_string(),
            content: "{{ invalid".to_string(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Syntax error"));
    }

    #[test]
    fn set_template_stores_valid_template() {
        let mut commander = MockCommander::new();
        commander
            .expect_validate_template()
            .with(eq("Hello {{ name }}"))
            .times(1)
            .returning(|_| Ok(()));

        let mut template_store = MockTemplateStore::new();
        template_store
            .expect_set_template_content()
            .with(eq("template"), eq("Hello {{ name }}".to_string()))
            .times(1)
            .return_const(());

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::SetTemplate {
            name: "template".to_string(),
            content: "Hello {{ name }}".to_string(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn set_values_validates_yaml() {
        let mut commander = MockCommander::new();
        commander
            .expect_parse_yaml()
            .with(eq("invalid: [yaml"))
            .times(1)
            .returning(|_| Err(ProvisionrError::YamlParse("YAML parse error".to_string())));

        let template_store = MockTemplateStore::new();
        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::SetValues {
            name: "template".to_string(),
            yaml: "invalid: [yaml".to_string(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn set_values_stores_valid_yaml() {
        let mut commander = MockCommander::new();
        commander
            .expect_parse_yaml()
            .with(eq("key: value"))
            .times(1)
            .returning(|s| {
                let docs = YamlLoader::load_from_str(s).unwrap();
                Ok(docs.into_iter().next().unwrap())
            });

        let mut template_store = MockTemplateStore::new();
        template_store
            .expect_set_values()
            .with(eq("template"), eq("key: value".to_string()))
            .times(1)
            .returning(|_, _| Ok(()));

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::SetValues {
            name: "template".to_string(),
            yaml: "key: value".to_string(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn render_returns_cached_content() {
        let commander = MockCommander::new();

        let mut template_store = MockTemplateStore::new();
        template_store.expect_get().with(eq("template")).times(1).returning(|_| {
            Some(TemplateData {
                template_content: "Hello {{ name }}".to_string(),
                id_field: "mac_address".to_string(),
                values_yaml: None,
                dynamic_fields: vec![],
                hashing_algorithm: HashingAlgorithm::None,
            })
        });

        let mut rendered_store = MockRenderedStore::new();
        rendered_store
            .expect_get_rendered()
            .with(eq("template"), eq("AA:BB:CC"))
            .times(1)
            .returning(|_, _| {
                Ok(Some(RenderedTemplate {
                    id: 1,
                    template_name: "template".to_string(),
                    id_field_value: "AA:BB:CC".to_string(),
                    rendered_content: "Cached Hello World".to_string(),
                    generated_values: "".to_string(),
                    created_at: "2024-01-01".to_string(),
                }))
            });

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        let mut query = HashMap::new();
        query.insert("mac_address".to_string(), "AA:BB:CC".to_string());
        handler.process_command(Command::RenderTemplate {
            name: "template".to_string(),
            query_values: query,
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert_eq!(result.unwrap(), "Cached Hello World");
    }

    #[test]
    fn render_generates_and_stores_new_content() {
        let mut commander = MockCommander::new();
        commander
            .expect_generate_dynamic_values()
            .times(1)
            .returning(|_, _| HashMap::new());
        commander
            .expect_map_to_yaml_string()
            .times(1)
            .returning(|_| Ok("---\n".to_string()));
        commander
            .expect_render_template()
            .withf(|template, values| {
                template == "Hello {{ name }}"
                    && values.get("name") == Some(&"World".to_string())
            })
            .times(1)
            .returning(|_, _| Ok("Hello World".to_string()));

        let mut template_store = MockTemplateStore::new();
        template_store.expect_get().with(eq("template")).times(1).returning(|_| {
            Some(TemplateData {
                template_content: "Hello {{ name }}".to_string(),
                id_field: "mac_address".to_string(),
                values_yaml: None,
                dynamic_fields: vec![],
                hashing_algorithm: HashingAlgorithm::None,
            })
        });

        let mut rendered_store = MockRenderedStore::new();
        rendered_store
            .expect_get_rendered()
            .times(1)
            .returning(|_, _| Ok(None));
        rendered_store
            .expect_store_rendered()
            .with(
                eq("template"),
                eq("AA:BB:CC"),
                eq("Hello World"),
                eq("---\n"),
            )
            .times(1)
            .returning(|_, _, _, _| Ok(1));

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        let mut query = HashMap::new();
        query.insert("mac_address".to_string(), "AA:BB:CC".to_string());
        query.insert("name".to_string(), "World".to_string());
        handler.process_command(Command::RenderTemplate {
            name: "template".to_string(),
            query_values: query,
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert_eq!(result.unwrap(), "Hello World");
    }

    #[test]
    fn render_fails_for_missing_template() {
        let commander = MockCommander::new();

        let mut template_store = MockTemplateStore::new();
        template_store
            .expect_get()
            .with(eq("missing"))
            .times(1)
            .returning(|_| None);

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::RenderTemplate {
            name: "missing".to_string(),
            query_values: HashMap::new(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn render_fails_for_missing_id_field() {
        let commander = MockCommander::new();

        let mut template_store = MockTemplateStore::new();
        template_store.expect_get().times(1).returning(|_| {
            Some(TemplateData {
                template_content: "Hello".to_string(),
                id_field: "mac_address".to_string(),
                values_yaml: None,
                dynamic_fields: vec![],
                hashing_algorithm: HashingAlgorithm::None,
            })
        });

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::RenderTemplate {
            name: "template".to_string(),
            query_values: HashMap::new(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required field"));
    }

    #[test]
    fn set_config_updates_store() {
        let commander = MockCommander::new();

        let mut template_store = MockTemplateStore::new();
        template_store
            .expect_set_config()
            .withf(|name, config| {
                name == "template"
                    && config.id_field == "serial_number"
                    && config.dynamic_fields.len() == 1
                    && config.dynamic_fields[0].field_name == "password"
                    && config.hashing_algorithm == HashingAlgorithm::Sha512
            })
            .times(1)
            .returning(|_, _| Ok(()));

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::SetConfig {
            name: "template".to_string(),
            config: TemplateConfig {
                id_field: "serial_number".to_string(),
                dynamic_fields: vec![DynamicFieldConfig {
                    field_name: "password".to_string(),
                    generator_type: GeneratorType::Alphanumeric { length: 16 },
                }],
                hashing_algorithm: HashingAlgorithm::Sha512,
            },
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_ok());
    }

    #[test]
    fn get_config_returns_template_config() {
        let commander = MockCommander::new();

        let mut template_store = MockTemplateStore::new();
        template_store
            .expect_get_config()
            .with(eq("template"))
            .times(1)
            .returning(|_| {
                Some(TemplateConfig {
                    id_field: "mac_address".to_string(),
                    dynamic_fields: vec![],
                    hashing_algorithm: HashingAlgorithm::None,
                })
            });

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::GetConfig {
            name: "template".to_string(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_ok());
        let config = result.unwrap().unwrap();
        assert_eq!(config.id_field, "mac_address");
    }

    #[test]
    fn delete_template_removes_from_store() {
        let commander = MockCommander::new();

        let mut template_store = MockTemplateStore::new();
        template_store
            .expect_delete()
            .with(eq("template"))
            .times(1)
            .return_const(());

        let rendered_store = MockRenderedStore::new();

        let mut handler = create_test_handler(commander, template_store, rendered_store);

        let (tx, rx) = oneshot::channel();
        handler.process_command(Command::DeleteTemplate {
            name: "template".to_string(),
            response: tx,
        });

        let result = rx.blocking_recv().unwrap();
        assert!(result.is_ok());
    }
}
