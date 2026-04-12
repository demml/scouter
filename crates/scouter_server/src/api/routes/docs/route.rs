use crate::api::state::AppState;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::{routing::get, Json, Router};
use scouter_types::contracts::ScouterServerError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Returns the largest byte index ≤ `i` that lies on a UTF-8 character boundary.
fn floor_char_boundary(s: &str, i: usize) -> usize {
    let mut idx = i.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

pub struct DocEntry {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub content: &'static str,
}

static DOCS: &[DocEntry] = &[
    // Overview
    DocEntry {
        id: "index",
        title: "Scouter Overview",
        category: "overview",
        content: include_str!("../../../../../../py-scouter/docs/index.md"),
    },
    DocEntry {
        id: "installation",
        title: "Installation",
        category: "setup",
        content: include_str!("../../../../../../py-scouter/docs/installation.md"),
    },
    DocEntry {
        id: "server",
        title: "Server Guide",
        category: "setup",
        content: include_str!("../../../../../../py-scouter/docs/server.md"),
    },
    // Agent evaluation
    DocEntry {
        id: "agents/overview",
        title: "Agent Evaluation Overview",
        category: "agents",
        content: include_str!("../../../../../../py-scouter/docs/docs/agents/overview.md"),
    },
    DocEntry {
        id: "agents/tasks",
        title: "Evaluation Tasks",
        category: "agents",
        content: include_str!("../../../../../../py-scouter/docs/docs/agents/tasks.md"),
    },
    DocEntry {
        id: "agents/offline-evaluation",
        title: "Offline Evaluation",
        category: "agents",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/agents/offline-evaluation.md"
        ),
    },
    DocEntry {
        id: "agents/online-evaluation",
        title: "Online Evaluation",
        category: "agents",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/agents/online-evaluation.md"
        ),
    },
    DocEntry {
        id: "agents/gates",
        title: "Conditional Gates",
        category: "agents",
        content: include_str!("../../../../../../py-scouter/docs/docs/agents/gates.md"),
    },
    DocEntry {
        id: "agents/eval-dataset",
        title: "Eval Dataset",
        category: "agents",
        content: include_str!("../../../../../../py-scouter/docs/docs/agents/eval-dataset.md"),
    },
    DocEntry {
        id: "agents/reading-results",
        title: "Reading Results",
        category: "agents",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/agents/reading-results.md"
        ),
    },
    // Monitoring
    DocEntry {
        id: "monitoring/index",
        title: "Monitoring Overview",
        category: "monitoring",
        content: include_str!("../../../../../../py-scouter/docs/docs/monitoring/index.md"),
    },
    DocEntry {
        id: "monitoring/inference",
        title: "Inference Monitoring",
        category: "monitoring",
        content: include_str!("../../../../../../py-scouter/docs/docs/monitoring/inference.md"),
    },
    DocEntry {
        id: "monitoring/psi/quickstart",
        title: "PSI Quickstart",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/psi/quickstart.md"
        ),
    },
    DocEntry {
        id: "monitoring/psi/theory",
        title: "PSI Theory",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/psi/theory.md"
        ),
    },
    DocEntry {
        id: "monitoring/psi/drift-config",
        title: "PSI Drift Config",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/psi/drift_config.md"
        ),
    },
    DocEntry {
        id: "monitoring/psi/drift-profile",
        title: "PSI Drift Profile",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/psi/drift_profile.md"
        ),
    },
    DocEntry {
        id: "monitoring/spc/quickstart",
        title: "SPC Quickstart",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/spc/quickstart.md"
        ),
    },
    DocEntry {
        id: "monitoring/spc/theory",
        title: "SPC Theory",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/spc/theory.md"
        ),
    },
    DocEntry {
        id: "monitoring/spc/drift-config",
        title: "SPC Drift Config",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/spc/drift_config.md"
        ),
    },
    DocEntry {
        id: "monitoring/spc/drift-profile",
        title: "SPC Drift Profile",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/spc/drift_profile.md"
        ),
    },
    DocEntry {
        id: "monitoring/custom/quickstart",
        title: "Custom Metrics Quickstart",
        category: "monitoring",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/monitoring/custom/quickstart.md"
        ),
    },
    // Distributed tracing
    DocEntry {
        id: "tracing/overview",
        title: "Tracing Overview",
        category: "tracing",
        content: include_str!("../../../../../../py-scouter/docs/docs/tracing/overview.md"),
    },
    DocEntry {
        id: "tracing/instrumentor",
        title: "Instrumentor Setup",
        category: "tracing",
        content: include_str!("../../../../../../py-scouter/docs/docs/tracing/instrumentor.md"),
    },
    DocEntry {
        id: "tracing/storage-architecture",
        title: "Storage Architecture",
        category: "tracing",
        content: include_str!(
            "../../../../../../py-scouter/docs/docs/tracing/storage-architecture.md"
        ),
    },
    // Server
    DocEntry {
        id: "server/index",
        title: "Server Overview",
        category: "server",
        content: include_str!("../../../../../../py-scouter/docs/docs/server/index.md"),
    },
    DocEntry {
        id: "server/postgres",
        title: "PostgreSQL Setup",
        category: "server",
        content: include_str!("../../../../../../py-scouter/docs/docs/server/postgres.md"),
    },
    // Profiling
    DocEntry {
        id: "profiling/overview",
        title: "Data Profiling",
        category: "profiling",
        content: include_str!("../../../../../../py-scouter/docs/docs/profiling/overview.md"),
    },
    // Bifrost (data archival)
    DocEntry {
        id: "bifrost/overview",
        title: "Bifrost Overview",
        category: "bifrost",
        content: include_str!("../../../../../../py-scouter/docs/docs/bifrost/overview.md"),
    },
    DocEntry {
        id: "bifrost/quickstart",
        title: "Bifrost Quickstart",
        category: "bifrost",
        content: include_str!("../../../../../../py-scouter/docs/docs/bifrost/quickstart.md"),
    },
    DocEntry {
        id: "bifrost/schema",
        title: "Bifrost Schema",
        category: "bifrost",
        content: include_str!("../../../../../../py-scouter/docs/docs/bifrost/schema.md"),
    },
    DocEntry {
        id: "bifrost/reading-data",
        title: "Reading Data",
        category: "bifrost",
        content: include_str!("../../../../../../py-scouter/docs/docs/bifrost/reading-data.md"),
    },
    DocEntry {
        id: "bifrost/writing-data",
        title: "Writing Data",
        category: "bifrost",
        content: include_str!("../../../../../../py-scouter/docs/docs/bifrost/writing-data.md"),
    },
    // API reference
    DocEntry {
        id: "api/stubs",
        title: "Python Type Stubs",
        category: "api",
        content: include_str!("../../../../../../py-scouter/docs/docs/api/stubs.md"),
    },
];

#[derive(Serialize, utoipa::ToSchema)]
pub struct DocSummary {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DocListResponse {
    pub docs: Vec<DocSummary>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DocResponse {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub content: &'static str,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct DocSearchQuery {
    /// Search term matched case-insensitively against doc titles and content. Max 200 characters.
    pub q: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DocSearchResult {
    pub id: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub snippet: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct DocSearchResponse {
    pub query: String,
    pub results: Vec<DocSearchResult>,
}

#[utoipa::path(
    get,
    path = "/scouter/api/v1/docs",
    responses(
        (status = 200, description = "List of all documentation entries", body = DocListResponse),
    ),
    tag = "docs"
)]
pub async fn list_docs() -> Json<DocListResponse> {
    let docs = DOCS
        .iter()
        .map(|e| DocSummary {
            id: e.id,
            title: e.title,
            category: e.category,
        })
        .collect();
    Json(DocListResponse { docs })
}

#[utoipa::path(
    get,
    path = "/scouter/api/v1/docs/search",
    params(DocSearchQuery),
    responses(
        (status = 200, description = "Matching docs with context snippets", body = DocSearchResponse),
        (status = 400, description = "Search query exceeds 200 characters", body = ScouterServerError),
    ),
    tag = "docs"
)]
pub async fn search_docs(
    Query(params): Query<DocSearchQuery>,
) -> Result<Json<DocSearchResponse>, (StatusCode, Json<ScouterServerError>)> {
    if params.q.len() > 200 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ScouterServerError {
                error: "Search query exceeds 200 character limit".to_string(),
                code: "BAD_REQUEST",
                suggested_action: None,
                retry: Some(false),
            }),
        ));
    }

    let q = params.q.to_lowercase();
    let results = DOCS
        .iter()
        .filter_map(|e| {
            let content_lower = e.content.to_lowercase();
            let title_lower = e.title.to_lowercase();
            if !title_lower.contains(&q) && !content_lower.contains(&q) {
                return None;
            }
            let snippet = if let Some(pos) = content_lower.find(&q) {
                let start = floor_char_boundary(&content_lower, pos.saturating_sub(80));
                let end = floor_char_boundary(
                    &content_lower,
                    (pos + q.len() + 80).min(content_lower.len()),
                );
                format!("...{}...", &content_lower[start..end])
            } else {
                e.title.to_string()
            };
            Some(DocSearchResult {
                id: e.id,
                title: e.title,
                category: e.category,
                snippet,
            })
        })
        .collect();

    Ok(Json(DocSearchResponse {
        query: params.q,
        results,
    }))
}

#[utoipa::path(
    get,
    path = "/scouter/api/v1/docs/{id}",
    params(
        ("id" = String, Path, description = "Slash-separated doc path, e.g. 'agents/overview' or 'monitoring/psi/quickstart'. Call GET /scouter/api/v1/docs to list all IDs.")
    ),
    responses(
        (status = 200, description = "Full documentation content in Markdown", body = DocResponse),
        (status = 404, description = "Doc not found", body = ScouterServerError),
    ),
    tag = "docs"
)]
pub async fn get_doc(
    Path(id): Path<String>,
) -> Result<Json<DocResponse>, (StatusCode, Json<ScouterServerError>)> {
    DOCS.iter()
        .find(|e| e.id == id.as_str())
        .map(|e| {
            Json(DocResponse {
                id: e.id,
                title: e.title,
                category: e.category,
                content: e.content,
            })
        })
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ScouterServerError {
                    error: format!("Doc '{id}' not found"),
                    code: "NOT_FOUND",
                    suggested_action: Some(
                        "Call GET /scouter/api/v1/docs to list all available doc IDs",
                    ),
                    retry: Some(false),
                }),
            )
        })
}

pub fn get_docs_router(prefix: &str) -> Router<Arc<AppState>> {
    Router::new()
        .route(&format!("{prefix}/api/v1/docs"), get(list_docs))
        .route(&format!("{prefix}/api/v1/docs/search"), get(search_docs))
        .route(&format!("{prefix}/api/v1/docs/{{*id}}"), get(get_doc))
}
