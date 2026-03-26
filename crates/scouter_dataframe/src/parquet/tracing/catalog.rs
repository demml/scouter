use async_trait::async_trait;
use dashmap::DashMap;
use datafusion::catalog::{CatalogProvider, SchemaProvider};
use datafusion::common::DataFusionError;
use datafusion::datasource::TableProvider;
use std::any::Any;
use std::sync::Arc;

/// Flat table registry for the tracing engine's "default" schema.
///
/// Backed by a `DashMap` — `DashMap::insert()` is a single atomic operation,
/// so concurrent readers either see the old `TableProvider` (already planning)
/// or the new one, but never "not found" during the swap window.
#[derive(Debug, Default)]
pub struct TraceSchemaProvider {
    tables: DashMap<String, Arc<dyn TableProvider>>,
}

impl TraceSchemaProvider {
    pub fn new() -> Self {
        Self {
            tables: DashMap::new(),
        }
    }

    /// Atomically swap the `TableProvider` for `name`.
    pub fn swap(&self, name: &str, provider: Arc<dyn TableProvider>) {
        self.tables.insert(name.to_string(), provider);
    }
}

#[async_trait]
impl SchemaProvider for TraceSchemaProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn table_names(&self) -> Vec<String> {
        self.tables.iter().map(|e| e.key().clone()).collect()
    }

    async fn table(
        &self,
        name: &str,
    ) -> Result<Option<Arc<dyn TableProvider>>, DataFusionError> {
        Ok(self.tables.get(name).map(|v| Arc::clone(v.value())))
    }

    fn table_exist(&self, name: &str) -> bool {
        self.tables.contains_key(name)
    }

    fn register_table(
        &self,
        name: String,
        table: Arc<dyn TableProvider>,
    ) -> datafusion::common::Result<Option<Arc<dyn TableProvider>>> {
        Ok(self.tables.insert(name, table))
    }

    fn deregister_table(
        &self,
        name: &str,
    ) -> datafusion::common::Result<Option<Arc<dyn TableProvider>>> {
        Ok(self.tables.remove(name).map(|(_, t)| t))
    }
}

/// Catalog provider for the tracing engine.
///
/// Wraps a single "default" schema backed by `TraceSchemaProvider` (DashMap).
/// Registered on the shared `SessionContext` as `"scouter_tracing"` — when the
/// session is configured with `with_default_catalog_and_schema("scouter_tracing", "default")`,
/// unqualified table names (`trace_spans`, `trace_summaries`) resolve through this
/// catalog's DashMap automatically. No SQL changes are needed in queries.
///
/// # Atomic swap invariant
///
/// Both `TraceSpanDBEngine` and `TraceSummaryDBEngine` call `swap()` to update
/// their respective table providers. A single `DashMap::insert()` is atomic —
/// there is no window where a planning query can see "table not found" between
/// the old provider being removed and the new one being registered.
#[derive(Debug)]
pub struct TraceCatalogProvider {
    schema: Arc<TraceSchemaProvider>,
}

impl TraceCatalogProvider {
    pub fn new() -> Self {
        Self {
            schema: Arc::new(TraceSchemaProvider::new()),
        }
    }

    /// Atomically swap the `TableProvider` for `name`.
    ///
    /// Use this instead of `ctx.deregister_table()` + `ctx.register_table()` —
    /// those two calls leave a window where the table appears absent.
    pub fn swap(&self, name: &str, provider: Arc<dyn TableProvider>) {
        self.schema.swap(name, provider);
    }
}

impl Default for TraceCatalogProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogProvider for TraceCatalogProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema_names(&self) -> Vec<String> {
        vec!["default".to_string()]
    }

    fn schema(&self, name: &str) -> Option<Arc<dyn SchemaProvider>> {
        if name == "default" {
            Some(Arc::clone(&self.schema) as Arc<dyn SchemaProvider>)
        } else {
            None
        }
    }
}
