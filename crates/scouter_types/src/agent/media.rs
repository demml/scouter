use base64::{Engine, engine::general_purpose::STANDARD};
use potato_head::prompt_types::{MediaKind, MediaRef, MediaSource};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[pyclass(from_py_object, eq, eq_int)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EvalMediaKind {
    Image,
    Document,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvalMediaSource {
    Url {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
    },
    Base64 {
        mime_type: String,
        data: String,
    },
}

#[pyclass(from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvalMedia {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub kind: EvalMediaKind,
    pub source: EvalMediaSource,
}

impl EvalMedia {
    pub fn into_media_ref(&self) -> MediaRef {
        let kind = match self.kind {
            EvalMediaKind::Image => MediaKind::Image,
            EvalMediaKind::Document => MediaKind::Document,
        };
        match &self.source {
            EvalMediaSource::Url { url, mime_type } => {
                MediaRef::new_url(kind, url.clone(), mime_type.clone())
            }
            EvalMediaSource::Base64 { mime_type, data } => {
                MediaRef::new_base64(kind, mime_type.clone(), data.clone())
            }
        }
    }

    pub fn decoded_byte_len(&self) -> usize {
        match &self.source {
            EvalMediaSource::Base64 { data, .. } => {
                let padding = data
                    .as_bytes()
                    .iter()
                    .rev()
                    .take_while(|byte| **byte == b'=')
                    .count();
                data.len() * 3 / 4 - padding
            }
            EvalMediaSource::Url { .. } => 0,
        }
    }
}

#[pyclass(from_py_object)]
#[derive(Debug, Clone)]
pub struct ImageMedia(pub EvalMedia);

#[pyclass(from_py_object)]
#[derive(Debug, Clone)]
pub struct DocumentMedia(pub EvalMedia);

impl ImageMedia {
    pub fn into_eval_media(self) -> EvalMedia {
        self.0
    }
}

impl DocumentMedia {
    pub fn into_eval_media(self) -> EvalMedia {
        self.0
    }
}

fn source_count(
    url: &Option<String>,
    data: &Option<&Bound<'_, PyBytes>>,
    path: &Option<PathBuf>,
) -> usize {
    usize::from(url.is_some()) + usize::from(data.is_some()) + usize::from(path.is_some())
}

fn source_from_path(media_ref: MediaRef) -> PyResult<EvalMediaSource> {
    match media_ref.source {
        MediaSource::Base64 { mime_type, data } => Ok(EvalMediaSource::Base64 { mime_type, data }),
        MediaSource::Url { .. } => Err(PyValueError::new_err(
            "path media resolution unexpectedly produced a URL source",
        )),
    }
}

fn build_media(
    id: String,
    kind: EvalMediaKind,
    url: Option<String>,
    data: Option<&Bound<'_, PyBytes>>,
    path: Option<PathBuf>,
    mime_type: Option<String>,
) -> PyResult<EvalMedia> {
    if source_count(&url, &data, &path) != 1 {
        return Err(PyValueError::new_err(
            "exactly one of url, bytes, or path must be provided",
        ));
    }

    let source = if let Some(url) = url {
        EvalMediaSource::Url { url, mime_type }
    } else if let Some(data) = data {
        let mime_type =
            mime_type.ok_or_else(|| PyValueError::new_err("bytes media requires mime_type"))?;
        EvalMediaSource::Base64 {
            mime_type,
            data: STANDARD.encode(data.as_bytes()),
        }
    } else if let Some(path) = path {
        match kind {
            EvalMediaKind::Image => source_from_path(MediaRef::image_path(path)?)?,
            EvalMediaKind::Document => source_from_path(MediaRef::document_path(path)?)?,
        }
    } else {
        unreachable!("source count validation guarantees one media source")
    };

    Ok(EvalMedia { id, kind, source })
}

#[pymethods]
impl ImageMedia {
    #[new]
    #[pyo3(signature = (id, *, url=None, bytes=None, path=None, mime_type=None))]
    pub fn new(
        id: String,
        url: Option<String>,
        bytes: Option<&Bound<'_, PyBytes>>,
        path: Option<PathBuf>,
        mime_type: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self(build_media(
            id,
            EvalMediaKind::Image,
            url,
            bytes,
            path,
            mime_type,
        )?))
    }

    pub fn __repr__(&self) -> String {
        format!("ImageMedia(id={:?})", self.0.id)
    }
}

#[pymethods]
impl DocumentMedia {
    #[new]
    #[pyo3(signature = (id, *, url=None, bytes=None, path=None, mime_type=None))]
    pub fn new(
        id: String,
        url: Option<String>,
        bytes: Option<&Bound<'_, PyBytes>>,
        path: Option<PathBuf>,
        mime_type: Option<String>,
    ) -> PyResult<Self> {
        Ok(Self(build_media(
            id,
            EvalMediaKind::Document,
            url,
            bytes,
            path,
            mime_type,
        )?))
    }

    pub fn __repr__(&self) -> String {
        format!("DocumentMedia(id={:?})", self.0.id)
    }
}

#[cfg(test)]
mod tests {
    use super::{EvalMedia, EvalMediaKind, EvalMediaSource};
    use base64::{Engine, engine::general_purpose::STANDARD};
    use potato_head::prompt_types::{MediaKind, MediaSource};

    #[test]
    fn eval_media_url_round_trip() {
        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: Some("image/png".to_string()),
            },
        };

        let json = serde_json::to_string(&media).unwrap();
        let decoded: EvalMedia = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, media);
    }

    #[test]
    fn eval_media_base64_round_trip() {
        let media = EvalMedia {
            id: "report".to_string(),
            kind: EvalMediaKind::Document,
            source: EvalMediaSource::Base64 {
                mime_type: "application/pdf".to_string(),
                data: STANDARD.encode(b"%PDF"),
            },
        };

        let json = serde_json::to_string(&media).unwrap();
        let decoded: EvalMedia = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded, media);
    }

    #[test]
    fn into_media_ref_url_image() {
        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: Some("image/png".to_string()),
            },
        };

        let media_ref = media.into_media_ref();

        assert_eq!(media_ref.kind, MediaKind::Image);
        assert_eq!(
            media_ref.source,
            MediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: Some("image/png".to_string())
            }
        );
    }

    #[test]
    fn into_media_ref_base64_document() {
        let data = STANDARD.encode(b"%PDF");
        let media = EvalMedia {
            id: "report".to_string(),
            kind: EvalMediaKind::Document,
            source: EvalMediaSource::Base64 {
                mime_type: "application/pdf".to_string(),
                data: data.clone(),
            },
        };

        let media_ref = media.into_media_ref();

        assert_eq!(media_ref.kind, MediaKind::Document);
        assert_eq!(
            media_ref.source,
            MediaSource::Base64 {
                mime_type: "application/pdf".to_string(),
                data
            }
        );
    }

    #[test]
    fn decoded_byte_len_url_is_zero() {
        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: None,
            },
        };

        assert_eq!(media.decoded_byte_len(), 0);
    }

    #[test]
    fn decoded_byte_len_base64_matches_input() {
        let input = vec![7_u8; 1024];
        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Base64 {
                mime_type: "image/png".to_string(),
                data: STANDARD.encode(input),
            },
        };

        assert!((1023..=1025).contains(&media.decoded_byte_len()));
    }
}
