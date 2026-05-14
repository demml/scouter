#![allow(dead_code)]

use crate::tiers::ObjectStoreCountSnapshot;
use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use futures::stream::BoxStream;
use object_store::path::Path;
use object_store::{
    CopyOptions, GetOptions, GetRange, GetResult, ListResult, MultipartUpload, ObjectMeta,
    ObjectStore, PutMultipartOptions, PutOptions, PutPayload, PutResult, Result,
};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct ObjectStoreCounts {
    pub list: AtomicU64,
    pub list_with_delimiter: AtomicU64,
    pub head: AtomicU64,
    pub get: AtomicU64,
    pub get_range: AtomicU64,
    pub put: AtomicU64,
    pub delete: AtomicU64,
    pub copy: AtomicU64,
    pub bytes: AtomicU64,
}

impl ObjectStoreCounts {
    pub fn snapshot(&self) -> ObjectStoreCountSnapshot {
        ObjectStoreCountSnapshot {
            list: self.list.load(Ordering::Relaxed),
            list_with_delimiter: self.list_with_delimiter.load(Ordering::Relaxed),
            head: self.head.load(Ordering::Relaxed),
            get: self.get.load(Ordering::Relaxed),
            get_range: self.get_range.load(Ordering::Relaxed),
            put: self.put.load(Ordering::Relaxed),
            delete: self.delete.load(Ordering::Relaxed),
            copy: self.copy.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug)]
pub struct CountingObjectStore<S> {
    inner: S,
    counts: Arc<ObjectStoreCounts>,
}

impl<S> CountingObjectStore<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            counts: Arc::new(ObjectStoreCounts::default()),
        }
    }

    pub fn counts(&self) -> Arc<ObjectStoreCounts> {
        Arc::clone(&self.counts)
    }

    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: ObjectStore> fmt::Display for CountingObjectStore<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CountingObjectStore({})", self.inner)
    }
}

#[async_trait]
impl<S: ObjectStore> ObjectStore for CountingObjectStore<S> {
    async fn put_opts(
        &self,
        location: &Path,
        payload: PutPayload,
        opts: PutOptions,
    ) -> Result<PutResult> {
        self.counts.put.fetch_add(1, Ordering::Relaxed);
        self.counts
            .bytes
            .fetch_add(payload.content_length() as u64, Ordering::Relaxed);
        self.inner.put_opts(location, payload, opts).await
    }

    async fn put_multipart_opts(
        &self,
        location: &Path,
        opts: PutMultipartOptions,
    ) -> Result<Box<dyn MultipartUpload>> {
        self.counts.put.fetch_add(1, Ordering::Relaxed);
        self.inner.put_multipart_opts(location, opts).await
    }

    async fn get_opts(&self, location: &Path, options: GetOptions) -> Result<GetResult> {
        if options.head {
            self.counts.head.fetch_add(1, Ordering::Relaxed);
        } else if options.range.is_some() {
            self.counts.get_range.fetch_add(1, Ordering::Relaxed);
        } else {
            self.counts.get.fetch_add(1, Ordering::Relaxed);
        }

        let requested_range = options.range.clone();
        let result = self.inner.get_opts(location, options).await?;
        let bytes = match requested_range {
            Some(GetRange::Bounded(range)) => range.end.saturating_sub(range.start),
            Some(GetRange::Offset(offset)) => result.meta.size.saturating_sub(offset),
            Some(GetRange::Suffix(suffix)) => suffix.min(result.meta.size),
            None if !result.range.is_empty() => result.range.end.saturating_sub(result.range.start),
            None => result.meta.size,
        };
        self.counts.bytes.fetch_add(bytes, Ordering::Relaxed);
        Ok(result)
    }

    async fn get_ranges(&self, location: &Path, ranges: &[Range<u64>]) -> Result<Vec<Bytes>> {
        self.counts
            .get_range
            .fetch_add(ranges.len() as u64, Ordering::Relaxed);
        self.counts.bytes.fetch_add(
            ranges
                .iter()
                .map(|range| range.end.saturating_sub(range.start))
                .sum(),
            Ordering::Relaxed,
        );
        self.inner.get_ranges(location, ranges).await
    }

    fn delete_stream(
        &self,
        locations: BoxStream<'static, Result<Path>>,
    ) -> BoxStream<'static, Result<Path>> {
        let counts = Arc::clone(&self.counts);
        self.inner
            .delete_stream(locations)
            .map(move |result| {
                if result.is_ok() {
                    counts.delete.fetch_add(1, Ordering::Relaxed);
                }
                result
            })
            .boxed()
    }

    fn list(&self, prefix: Option<&Path>) -> BoxStream<'static, Result<ObjectMeta>> {
        self.counts.list.fetch_add(1, Ordering::Relaxed);
        self.inner.list(prefix)
    }

    async fn list_with_delimiter(&self, prefix: Option<&Path>) -> Result<ListResult> {
        self.counts
            .list_with_delimiter
            .fetch_add(1, Ordering::Relaxed);
        self.inner.list_with_delimiter(prefix).await
    }

    async fn copy_opts(&self, from: &Path, to: &Path, options: CopyOptions) -> Result<()> {
        self.counts.copy.fetch_add(1, Ordering::Relaxed);
        self.inner.copy_opts(from, to, options).await
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;
    use object_store::ObjectStoreExt;
    use object_store::memory::InMemory;

    #[tokio::test]
    async fn counts_basic_operations() {
        let store = CountingObjectStore::new(InMemory::new());
        let path = Path::from("bench/counts.txt");

        store
            .put(&path, PutPayload::from_static(b"abcdef"))
            .await
            .unwrap();
        let _ = store.head(&path).await.unwrap();
        let _ = store.get_range(&path, 1..3).await.unwrap();
        let _ = store.list(None).collect::<Vec<_>>().await;
        store.delete(&path).await.unwrap();

        let snapshot = store.counts().snapshot();
        assert_eq!(snapshot.put, 1);
        assert_eq!(snapshot.head, 1);
        assert_eq!(snapshot.get_range, 1);
        assert_eq!(snapshot.list, 1);
        assert_eq!(snapshot.delete, 1);
        assert!(snapshot.bytes >= 8);
    }
}
