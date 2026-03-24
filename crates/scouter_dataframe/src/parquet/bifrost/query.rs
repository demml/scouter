use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Tracks active queries for cancellation support.
#[derive(Default)]
pub struct QueryTracker {
    active: RwLock<HashMap<String, CancellationToken>>,
}

impl QueryTracker {
    pub fn new() -> Self {
        Self {
            active: RwLock::new(HashMap::new()),
        }
    }

    /// Register a query and return a cancellation token.
    /// The caller should poll `token.cancelled()` in a `tokio::select!`.
    /// Returns `Err(DuplicateQueryId)` if the query_id is already registered.
    pub async fn register(
        &self,
        query_id: &str,
    ) -> Result<CancellationToken, crate::error::DatasetEngineError> {
        let mut active = self.active.write().await;
        if active.contains_key(query_id) {
            return Err(crate::error::DatasetEngineError::DuplicateQueryId(
                query_id.to_string(),
            ));
        }
        let token = CancellationToken::new();
        active.insert(query_id.to_string(), token.clone());
        Ok(token)
    }

    /// Cancel a running query. Returns `true` if the query was found and cancelled.
    pub async fn cancel(&self, query_id: &str) -> bool {
        if let Some(token) = self.active.write().await.remove(query_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Remove a completed query from tracking.
    pub async fn remove(&self, query_id: &str) {
        self.active.write().await.remove(query_id);
    }
}

/// Result of an executed query with metadata.
pub struct QueryResult {
    pub batches: Vec<arrow_array::RecordBatch>,
    pub metadata: QueryExecutionMetadata,
}

/// Metadata about query execution.
#[derive(Debug, Clone)]
pub struct QueryExecutionMetadata {
    pub query_id: String,
    pub rows_returned: u64,
    pub truncated: bool,
    pub execution_time_ms: u64,
    pub bytes_scanned: Option<u64>,
}
