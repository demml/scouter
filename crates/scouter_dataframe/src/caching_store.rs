use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use futures::StreamExt;
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
        self.inner.put_opts(location, payload, opts).await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> Result<Box<dyn MultipartUpload>> {
        self.inner.put_multipart_opts(location, opts).await
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
        let key: Arc<str> = location.to_string().into();

        if options.head && options.range.is_none() && is_plain_request(&options) {
            if let Some(meta) = self.head_cache.get(&key) {
                return Ok(GetResult {
                    payload: GetResultPayload::Stream(futures::stream::empty().boxed()),
                    meta,
                    range: 0..0,
                    attributes: Default::default(),
                });
            }

            let result = self.inner.get_opts(location, options).await?;
            self.head_cache.insert(key, result.meta.clone());
            return Ok(result);
        }

        if let Some(GetRange::Bounded(range)) = options.range.as_ref() {
            let len = range.end.saturating_sub(range.start);
            if !options.head && is_plain_request(&options) && len <= MAX_CACHEABLE_BYTES {
                let meta = match self.head_cache.get(&key) {
                    Some(meta) => meta,
                    None => {
                        let meta = self.inner.head(location).await?;
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
                        return Ok(GetResult {
                            payload: GetResultPayload::Stream(
                                futures::stream::once(async move { Ok(bytes) }).boxed(),
                            ),
                            meta,
                            range: range.clone(),
                            attributes: Default::default(),
                        });
                    }

                    let bytes = self.inner.get_range(location, range.clone()).await?;
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

        self.inner.get_opts(location, options).await
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
                    let key: Arc<str> = location.to_string().into();
                    head_cache.invalidate(&key);
                    range_cache.invalidate_all();
                }
                result
            })
            .boxed()
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>> {
        self.inner.list(prefix)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        self.inner.list_with_delimiter(prefix).await
    }

    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        self.inner.copy_opts(from, to, options).await?;
        let to_key: Arc<str> = to.to_string().into();
        self.head_cache.invalidate(&to_key);
        self.range_cache.invalidate_all();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;
    use object_store::PutPayload;

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
