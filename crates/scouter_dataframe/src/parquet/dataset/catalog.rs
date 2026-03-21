use async_trait::async_trait;
use dashmap::DashMap;
use datafusion::catalog::{CatalogProvider, SchemaProvider};
use datafusion::common::DataFusionError;
use datafusion::datasource::TableProvider;
use scouter_types::dataset::DatasetNamespace;
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// Custom DataFusion catalog provider that maps the first level of the
/// three-level `catalog.schema.table` SQL namespace.
///
/// Wraps a `DashMap` of schema names → `DatasetSchemaProvider` instances.
/// Registered on the shared `query_ctx` so DataFusion can resolve table
/// references in SQL queries.
#[derive(Debug)]
pub struct DatasetCatalogProvider {
    schemas: DashMap<String, Arc<DatasetSchemaProvider>>,
}

impl DatasetCatalogProvider {
    pub fn new() -> Self {
        Self {
            schemas: DashMap::new(),
        }
    }

    /// Get or create a schema provider for the given schema name.
    pub fn get_or_create_schema(&self, schema_name: &str) -> Arc<DatasetSchemaProvider> {
        self.schemas
            .entry(schema_name.to_string())
            .or_insert_with(|| Arc::new(DatasetSchemaProvider::new()))
            .clone()
    }

    /// Atomically swap the `TableProvider` for a table after a Delta write.
    /// In-flight queries that already obtained a `DataFrame` hold a reference
    /// to the old snapshot and complete normally.
    pub fn swap_table(
        &self,
        namespace: &DatasetNamespace,
        provider: Arc<dyn TableProvider>,
    ) {
        let schema = self.get_or_create_schema(&namespace.schema_name);
        schema.tables.insert(namespace.table.clone(), provider);
    }

    /// Remove a table from the catalog (used during TTL eviction).
    pub fn remove_table(&self, namespace: &DatasetNamespace) {
        if let Some(schema) = self.schemas.get(&namespace.schema_name) {
            schema.tables.remove(&namespace.table);
        }
    }

    /// Check if a table exists in the catalog.
    pub fn has_table(&self, namespace: &DatasetNamespace) -> bool {
        self.schemas
            .get(&namespace.schema_name)
            .map(|s| s.tables.contains_key(&namespace.table))
            .unwrap_or(false)
    }
}

impl Default for DatasetCatalogProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CatalogProvider for DatasetCatalogProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema_names(&self) -> Vec<String> {
        self.schemas.iter().map(|e| e.key().clone()).collect()
    }

    fn schema(&self, name: &str) -> Option<Arc<dyn SchemaProvider>> {
        self.schemas
            .get(name)
            .map(|s| Arc::clone(&*s) as Arc<dyn SchemaProvider>)
    }

    fn register_schema(
        &self,
        name: &str,
        schema: Arc<dyn SchemaProvider>,
    ) -> datafusion::common::Result<Option<Arc<dyn SchemaProvider>>> {
        let dataset_schema = schema
            .as_any()
            .downcast_ref::<DatasetSchemaProvider>()
            .ok_or_else(|| {
                DataFusionError::Internal("Expected DatasetSchemaProvider".to_string())
            })?;
        let prev = self
            .schemas
            .insert(name.to_string(), Arc::new(dataset_schema.clone()));
        Ok(prev.map(|p| p as Arc<dyn SchemaProvider>))
    }
}

/// Custom DataFusion schema provider that maps the second level of the
/// three-level namespace. Holds `DashMap<table_name, TableProvider>`.
#[derive(Debug, Clone)]
pub struct DatasetSchemaProvider {
    tables: DashMap<String, Arc<dyn TableProvider>>,
}

impl DatasetSchemaProvider {
    pub fn new() -> Self {
        Self {
            tables: DashMap::new(),
        }
    }
}

impl Default for DatasetSchemaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SchemaProvider for DatasetSchemaProvider {
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
        Ok(self.tables.get(name).map(|t| Arc::clone(&*t)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catalog_schema_lifecycle() {
        let catalog = DatasetCatalogProvider::new();

        // Initially empty
        assert!(catalog.schema_names().is_empty());
        assert!(catalog.schema("test_schema").is_none());

        // Get or create a schema
        let schema = catalog.get_or_create_schema("test_schema");
        assert!(catalog.schema_names().contains(&"test_schema".to_string()));
        assert!(schema.table_names().is_empty());

        // Getting the same schema again returns the same instance
        let schema2 = catalog.get_or_create_schema("test_schema");
        assert_eq!(schema.table_names(), schema2.table_names());
    }

    #[test]
    fn test_catalog_has_table() {
        let catalog = DatasetCatalogProvider::new();
        let ns = DatasetNamespace::new("cat", "sch", "tbl").unwrap();

        assert!(!catalog.has_table(&ns));

        // Add a table via swap_table
        let schema = arrow::datatypes::Schema::new(vec![arrow::datatypes::Field::new(
            "id",
            arrow::datatypes::DataType::Int64,
            false,
        )]);
        let batch = arrow_array::RecordBatch::new_empty(Arc::new(schema));
        let provider = Arc::new(
            datafusion::datasource::MemTable::try_new(
                batch.schema(),
                vec![vec![batch]],
            )
            .unwrap(),
        );
        catalog.swap_table(&ns, provider);

        assert!(catalog.has_table(&ns));

        // Remove it
        catalog.remove_table(&ns);
        assert!(!catalog.has_table(&ns));
    }
}
