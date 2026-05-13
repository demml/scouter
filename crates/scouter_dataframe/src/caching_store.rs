use crate::parquet::utils::{
    OBJECT_STORE_OPERATION_COPY, OBJECT_STORE_OPERATION_DELETE, OBJECT_STORE_OPERATION_GET_RANGE,
    OBJECT_STORE_OPERATION_LIST, OBJECT_STORE_OPERATION_LIST_WITH_DELIMITER,
    OBJECT_STORE_OPERATION_PUT, ObjectStoreRequestTelemetry, get_options_range,
    observe_object_meta_stream, observed_get_result_bytes,
};
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use futures::stream::BoxStream;
use mini_moka::sync::Cache;
use object_store::path::Path;
use object_store::{
    CopyOptions, GetOptions, GetRange, GetResult, GetResultPayload, ListResult, MultipartUpload,
    ObjectMeta, ObjectStore, ObjectStoreExt, PutMultipartOptions, PutOptions, PutPayload,
    PutResult, Result,
};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tracing::Instrument;

/// Cache key for range reads: (path, start, end).
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct RangeCacheKey {
    path: Arc<str>,
    start: u64,
    end: u64,
}

/// Maximum size of a single range read that will be cached (2 MB).
/// Parquet footers are typically well under this; column data reads are larger
/// and will pass through uncached.
const MAX_CACHEABLE_BYTES: u64 = 2 * 1024 * 1024;
const CACHING_STORE_BACKEND: &str = "cache";

/// An `ObjectStore` wrapper that caches `head()` and small `get_range()` responses.
///
/// After Z-ORDER compaction the Parquet files backing Delta tables are immutable:
/// the same path always returns the same bytes. Caching the metadata calls that
/// DataFusion issues on every query (HEAD for file size, then GET-range for the
/// footer) eliminates redundant cloud round-trips.
///
/// All mutating and streaming methods delegate directly to the inner store.
#[derive(Debug)]
pub struct CachingStore<T: ObjectStore> {
    inner: T,
    /// path → ObjectMeta
    head_cache: Cache<Arc<str>, ObjectMeta>,
    /// (path, start, end) → Bytes
    range_cache: Cache<RangeCacheKey, Bytes>,
}

impl<T: ObjectStore> CachingStore<T> {
    /// Create a new caching wrapper.
    ///
    /// * `inner` – the concrete object store to wrap.
    /// * `range_cache_max_bytes` – maximum total weight of the range cache
    ///   (each entry is weighed by its byte length).
    pub fn new(inner: T, range_cache_max_bytes: u64) -> Self {
        let ttl = Duration::from_secs(3600); // 1 hour

        let head_cache = Cache::builder()
            .max_capacity(10_000)
            .time_to_live(ttl)
            .build();

        let range_cache = Cache::builder()
            .max_capacity(range_cache_max_bytes)
            .weigher(|_key: &RangeCacheKey, value: &Bytes| -> u32 {
                // Clamp to u32::MAX for the weigher contract.
                value.len().min(u32::MAX as usize) as u32
            })
            .time_to_live(ttl)
            .build();

        Self {
            inner,
            head_cache,
            range_cache,
        }
    }
}

fn is_plain_request(options: &GetOptions) -> bool {
    options.if_match.is_none()
        && options.if_none_match.is_none()
        && options.if_modified_since.is_none()
        && options.if_unmodified_since.is_none()
        && options.version.is_none()
        && options.extensions.is_empty()
}

impl<T: ObjectStore> fmt::Display for CachingStore<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CachingStore({})", self.inner)
    }
}

#[async_trait]
impl<T: ObjectStore> ObjectStore for CachingStore<T> {
    // ── Passthrough (mutating / streaming) ──────────────────────────────

    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> Result<PutResult> {
        let bytes = payload.content_length() as u64;
        let telemetry = ObjectStoreRequestTelemetry::new(
            CACHING_STORE_BACKEND,
            OBJECT_STORE_OPERATION_PUT,
            Some(location),
            None,
            Some(bytes),
            None,
        );
        let result = self
            .inner
            .put_opts(location, payload, opts)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(_) => telemetry.finish_success(bytes),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> Result<Box<dyn MultipartUpload>> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            CACHING_STORE_BACKEND,
            OBJECT_STORE_OPERATION_PUT,
            Some(location),
            None,
            None,
            None,
        );
        let result = self
            .inner
            .put_multipart_opts(location, opts)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(_) => telemetry.finish_success(0),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
        let key: Arc<str> = location.to_string().into();
        let operation = if options.head && options.range.is_none() {
            crate::parquet::utils::OBJECT_STORE_OPERATION_HEAD
        } else if options.range.is_some() {
            OBJECT_STORE_OPERATION_GET_RANGE
        } else {
            crate::parquet::utils::OBJECT_STORE_OPERATION_GET
        };
        let (range_start, range_len) = get_options_range(&options);

        if options.head && options.range.is_none() && is_plain_request(&options) {
            if let Some(meta) = self.head_cache.get(&key) {
                let telemetry = ObjectStoreRequestTelemetry::new(
                    CACHING_STORE_BACKEND,
                    operation,
                    Some(location),
                    range_start,
                    range_len,
                    Some(true),
                );
                telemetry.finish_success(0);
                return Ok(GetResult {
                    payload: GetResultPayload::Stream(futures::stream::empty().boxed()),
                    meta,
                    range: 0..0,
                    attributes: Default::default(),
                });
            }

            let telemetry = ObjectStoreRequestTelemetry::new(
                CACHING_STORE_BACKEND,
                operation,
                Some(location),
                range_start,
                range_len,
                Some(false),
            );
            let result = self
                .inner
                .get_opts(location, options)
                .instrument(telemetry.span())
                .await;
            match &result {
                Ok(result) => {
                    telemetry.finish_success(observed_get_result_bytes(operation, result))
                }
                Err(error) => telemetry.finish_error(error),
            }
            let result = result?;
            self.head_cache.insert(key, result.meta.clone());
            return Ok(result);
        }

        if let Some(GetRange::Bounded(range)) = options.range.as_ref() {
            let len = range.end.saturating_sub(range.start);
            if !options.head && is_plain_request(&options) && len <= MAX_CACHEABLE_BYTES {
                let meta = match self.head_cache.get(&key) {
                    Some(meta) => meta,
                    None => {
                        let head_telemetry = ObjectStoreRequestTelemetry::new(
                            CACHING_STORE_BACKEND,
                            crate::parquet::utils::OBJECT_STORE_OPERATION_HEAD,
                            Some(location),
                            None,
                            None,
                            Some(false),
                        );
                        let meta = self
                            .inner
                            .head(location)
                            .instrument(head_telemetry.span())
                            .await;
                        match &meta {
                            Ok(_) => head_telemetry.finish_success(0),
                            Err(error) => head_telemetry.finish_error(error),
                        }
                        let meta = meta?;
                        self.head_cache.insert(key.clone(), meta.clone());
                        meta
                    }
                };

                if range.end <= meta.size {
                    let range_key = RangeCacheKey {
                        path: key,
                        start: range.start,
                        end: range.end,
                    };

                    if let Some(bytes) = self.range_cache.get(&range_key) {
                        let telemetry = ObjectStoreRequestTelemetry::new(
                            CACHING_STORE_BACKEND,
                            operation,
                            Some(location),
                            range_start,
                            range_len,
                            Some(true),
                        );
                        telemetry.finish_success(bytes.len() as u64);
                        return Ok(GetResult {
                            payload: GetResultPayload::Stream(
                                futures::stream::once(async move { Ok(bytes) }).boxed(),
                            ),
                            meta,
                            range: range.clone(),
                            attributes: Default::default(),
                        });
                    }

                    let telemetry = ObjectStoreRequestTelemetry::new(
                        CACHING_STORE_BACKEND,
                        operation,
                        Some(location),
                        range_start,
                        range_len,
                        Some(false),
                    );
                    let bytes = self
                        .inner
                        .get_range(location, range.clone())
                        .instrument(telemetry.span())
                        .await;
                    match &bytes {
                        Ok(bytes) => telemetry.finish_success(bytes.len() as u64),
                        Err(error) => telemetry.finish_error(error),
                    }
                    let bytes = bytes?;
                    self.range_cache.insert(range_key, bytes.clone());
                    return Ok(GetResult {
                        payload: GetResultPayload::Stream(
                            futures::stream::once(async move { Ok(bytes) }).boxed(),
                        ),
                        meta,
                        range: range.clone(),
                        attributes: Default::default(),
                    });
                }
            }
        }

        let telemetry = ObjectStoreRequestTelemetry::new(
            CACHING_STORE_BACKEND,
            operation,
            Some(location),
            range_start,
            range_len,
            Some(false),
        );
        let result = self
            .inner
            .get_opts(location, options)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(result) => telemetry.finish_success(observed_get_result_bytes(operation, result)),
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, Result<Path>>,
    ) -> BoxStream<'static, Result<Path>> {
        let head_cache = self.head_cache.clone();
        let range_cache = self.range_cache.clone();
        self.inner
            .delete_stream(locations)
            .map(move |result| {
                if let Ok(location) = &result {
                    let telemetry = ObjectStoreRequestTelemetry::new(
                        CACHING_STORE_BACKEND,
                        OBJECT_STORE_OPERATION_DELETE,
                        Some(location),
                        None,
                        None,
                        None,
                    );
                    let _entered = telemetry.enter();
                    telemetry.finish_success(0);
                    let key: Arc<str> = location.to_string().into();
                    head_cache.invalidate(&key);
                    range_cache.invalidate_all();
                }
                result
            })
            .boxed()
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            CACHING_STORE_BACKEND,
            OBJECT_STORE_OPERATION_LIST,
            prefix,
            None,
            None,
            None,
        );
        observe_object_meta_stream(self.inner.list(prefix), telemetry)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            CACHING_STORE_BACKEND,
            OBJECT_STORE_OPERATION_LIST_WITH_DELIMITER,
            prefix,
            None,
            None,
            None,
        );
        let result = self
            .inner
            .list_with_delimiter(prefix)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(result) => {
                let object_bytes = result
                    .objects
                    .iter()
                    .fold(0_u64, |bytes, meta| bytes.saturating_add(meta.size));
                telemetry.finish_success(object_bytes);
            }
            Err(error) => telemetry.finish_error(error),
        }
        result
    }

    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        let telemetry = ObjectStoreRequestTelemetry::new(
            CACHING_STORE_BACKEND,
            OBJECT_STORE_OPERATION_COPY,
            Some(from),
            None,
            None,
            None,
        );
        let result = self
            .inner
            .copy_opts(from, to, options)
            .instrument(telemetry.span())
            .await;
        match &result {
            Ok(_) => telemetry.finish_success(0),
            Err(error) => telemetry.finish_error(error),
        }
        result?;
        let to_key: Arc<str> = to.to_string().into();
        self.head_cache.invalidate(&to_key);
        self.range_cache.invalidate_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::PutPayload;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn head_is_cached() {
        let mem = InMemory::new();
        let path = Path::from("test/file.parquet");
        mem.put(&path, PutPayload::from_static(b"hello"))
            .await
            .unwrap();

        let store = CachingStore::new(mem, 64 * 1024 * 1024);

        // First call populates the cache.
        let meta1 = store.head(&path).await.unwrap();
        // Second call should return cached value.
        let meta2 = store.head(&path).await.unwrap();

        assert_eq!(meta1.size, meta2.size);
        assert_eq!(meta1.location, meta2.location);
    }

    #[tokio::test]
    async fn get_range_is_cached() {
        let mem = InMemory::new();
        let path = Path::from("test/file.parquet");
        let data = b"0123456789abcdef";
        mem.put(&path, PutPayload::from_static(data)).await.unwrap();

        let store = CachingStore::new(mem, 64 * 1024 * 1024);

        let bytes1 = store.get_range(&path, 4..10).await.unwrap();
        let bytes2 = store.get_range(&path, 4..10).await.unwrap();

        assert_eq!(bytes1, bytes2);
        assert_eq!(&bytes1[..], b"456789");
    }

    #[tokio::test]
    async fn large_range_not_cached() {
        let mem = InMemory::new();
        let path = Path::from("test/big.parquet");
        let data = vec![0u8; 3 * 1024 * 1024]; // 3 MB — exceeds MAX_CACHEABLE_BYTES
        mem.put(&path, PutPayload::from(data)).await.unwrap();

        let store = CachingStore::new(mem, 64 * 1024 * 1024);

        // Should still work, just not be cached.
        let bytes = store.get_range(&path, 0..3 * 1024 * 1024).await.unwrap();
        assert_eq!(bytes.len(), 3 * 1024 * 1024);
    }
}
