use crate::error::ProvisionrError;
use crate::storage::models::{RenderedTemplate, RenderedTemplateSummary};
use rusqlite::{params, Connection, Result as SqliteResult};

#[cfg_attr(test, mockall::automock)]
pub trait RenderedStore: Send {
    fn init(&self) -> Result<(), ProvisionrError>;
    fn store_rendered(
        &self,
        template_name: &str,
        id_field_value: &str,
        rendered_content: &str,
        generated_values: &str,
    ) -> Result<i64, ProvisionrError>;
    fn get_rendered(
        &self,
        template_name: &str,
        id_field_value: &str,
    ) -> Result<Option<RenderedTemplate>, ProvisionrError>;
    fn list_rendered(&self, template_name: &str) -> Result<Vec<RenderedTemplateSummary>, ProvisionrError>;
}

pub struct SqliteRenderedStore {
    conn: Connection,
}

impl SqliteRenderedStore {
    pub fn new(path: &str) -> Result<Self, String> {
        let conn =
            Connection::open(path).map_err(|e| format!("Failed to open database: {}", e))?;
        Ok(Self { conn })
    }
}

impl RenderedStore for SqliteRenderedStore {
    fn init(&self) -> Result<(), ProvisionrError> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS rendered_templates (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    template_name TEXT NOT NULL,
                    id_field_value TEXT NOT NULL,
                    rendered_content TEXT NOT NULL,
                    generated_values TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT (datetime('now')),
                    UNIQUE(template_name, id_field_value)
                )",
                [],
            )
            .map_err(|e| ProvisionrError::Database(format!("Failed to create table: {}", e)))?;

        self.conn
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_template_name ON rendered_templates(template_name)",
                [],
            )
            .map_err(|e| ProvisionrError::Database(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    fn store_rendered(
        &self,
        template_name: &str,
        id_field_value: &str,
        rendered_content: &str,
        generated_values: &str,
    ) -> Result<i64, ProvisionrError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO rendered_templates
                 (template_name, id_field_value, rendered_content, generated_values, created_at)
                 VALUES (?1, ?2, ?3, ?4, datetime('now'))",
                params![template_name, id_field_value, rendered_content, generated_values],
            )
            .map_err(|e| ProvisionrError::Database(format!("Failed to insert rendered template: {}", e)))?;

        Ok(self.conn.last_insert_rowid())
    }

    fn get_rendered(
        &self,
        template_name: &str,
        id_field_value: &str,
    ) -> Result<Option<RenderedTemplate>, ProvisionrError> {
        let result: SqliteResult<RenderedTemplate> = self.conn.query_row(
            "SELECT id, template_name, id_field_value, rendered_content, generated_values, created_at
             FROM rendered_templates
             WHERE template_name = ?1 AND id_field_value = ?2",
            params![template_name, id_field_value],
            |row| {
                Ok(RenderedTemplate {
                    id: row.get(0)?,
                    template_name: row.get(1)?,
                    id_field_value: row.get(2)?,
                    rendered_content: row.get(3)?,
                    generated_values: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        );

        match result {
            Ok(template) => Ok(Some(template)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(ProvisionrError::Database(format!("Database query failed: {}", e))),
        }
    }

    fn list_rendered(&self, template_name: &str) -> Result<Vec<RenderedTemplateSummary>, ProvisionrError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id_field_value, created_at
                 FROM rendered_templates
                 WHERE template_name = ?1
                 ORDER BY created_at DESC",
            )
            .map_err(|e| ProvisionrError::Database(format!("Failed to prepare statement: {}", e)))?;

        let rows = stmt
            .query_map(params![template_name], |row| {
                Ok(RenderedTemplateSummary {
                    id_field_value: row.get(0)?,
                    created_at: row.get(1)?,
                })
            })
            .map_err(|e| ProvisionrError::Database(format!("Query failed: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| ProvisionrError::Database(format!("Row error: {}", e)))?);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_store() -> SqliteRenderedStore {
        let store = SqliteRenderedStore::new(":memory:").expect("in-memory database should open");
        store.init().expect("schema should initialise");
        store
    }

    #[test]
    fn store_then_get_round_trips_all_fields() {
        let store = memory_store();
        let id = store
            .store_rendered("kickstart", "aa:bb:cc", "rendered body", r#"{"pw":"x"}"#)
            .unwrap();

        let row = store.get_rendered("kickstart", "aa:bb:cc").unwrap().expect("row should exist");

        assert_eq!(row.id, id);
        assert_eq!(row.template_name, "kickstart");
        assert_eq!(row.id_field_value, "aa:bb:cc");
        assert_eq!(row.rendered_content, "rendered body");
        assert_eq!(row.generated_values, r#"{"pw":"x"}"#);
        assert!(!row.created_at.is_empty());
    }

    #[test]
    fn get_missing_row_returns_none() {
        let store = memory_store();
        assert!(store.get_rendered("kickstart", "missing").unwrap().is_none());
    }

    #[test]
    fn storing_same_key_replaces_previous_row() {
        let store = memory_store();
        store.store_rendered("kickstart", "host1", "first", "{}").unwrap();
        store.store_rendered("kickstart", "host1", "second", "{}").unwrap();

        let row = store.get_rendered("kickstart", "host1").unwrap().unwrap();
        assert_eq!(row.rendered_content, "second");
        assert_eq!(store.list_rendered("kickstart").unwrap().len(), 1);
    }

    #[test]
    fn list_rendered_only_returns_matching_template() {
        let store = memory_store();
        store.store_rendered("kickstart", "host1", "a", "{}").unwrap();
        store.store_rendered("kickstart", "host2", "b", "{}").unwrap();
        store.store_rendered("cloud-init", "host3", "c", "{}").unwrap();

        let mut ids: Vec<String> = store
            .list_rendered("kickstart")
            .unwrap()
            .into_iter()
            .map(|s| s.id_field_value)
            .collect();
        ids.sort();

        assert_eq!(ids, ["host1", "host2"]);
        assert!(store.list_rendered("unknown").unwrap().is_empty());
    }

    #[test]
    fn existing_database_file_is_readable_after_reopen() {
        let path = std::env::temp_dir().join(format!("provisionr-sqlite-test-{}.db", std::process::id()));
        let path_str = path.to_str().unwrap();
        let _ = std::fs::remove_file(&path);

        {
            let store = SqliteRenderedStore::new(path_str).unwrap();
            store.init().unwrap();
            store.store_rendered("kickstart", "host1", "persisted", "{}").unwrap();
        }

        let reopened = SqliteRenderedStore::new(path_str).unwrap();
        reopened.init().expect("init should be idempotent on an existing database");
        let row = reopened.get_rendered("kickstart", "host1").unwrap().unwrap();
        assert_eq!(row.rendered_content, "persisted");

        drop(reopened);
        std::fs::remove_file(&path).unwrap();
    }
}
