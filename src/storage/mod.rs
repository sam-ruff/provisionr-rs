pub mod evmap_store;
pub mod models;
pub mod sqlite_store;

pub use evmap_store::{EvmapTemplateStore, TemplateStore};
pub use sqlite_store::{RenderedStore, SqliteRenderedStore};

#[cfg(test)]
pub use evmap_store::MockTemplateStore;
#[cfg(test)]
pub use sqlite_store::MockRenderedStore;
