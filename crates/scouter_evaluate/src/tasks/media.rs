use potato_head::prompt_types::MediaRef;
use potato_head::{Task, Workflow};
use scouter_types::agent::media::EvalMedia;
use serde_json::Value;
use std::sync::{Arc, RwLock};

use crate::error::EvaluationError;

pub fn build_media_refs(media: &[EvalMedia]) -> Vec<(String, MediaRef)> {
    media
        .iter()
        .map(|em| (em.id.clone(), em.into_media_ref()))
        .collect()
}

/// Executes a task with per-record media bindings by cloning the task before
/// binding. This bypasses Workflow::execute_task response-schema retries.
pub async fn execute_task_with_media(
    workflow: &Workflow,
    task_id: &str,
    context: &Value,
    bindings: &[(String, MediaRef)],
) -> Result<Value, EvaluationError> {
    let task_arc = workflow
        .task_list
        .get_task(task_id)
        .ok_or_else(|| EvaluationError::TaskNotFound(task_id.to_string()))?;

    let mut task_clone: Task = task_arc
        .read()
        .map_err(|_| EvaluationError::TaskLockError)?
        .clone();

    for (id, media_ref) in bindings {
        task_clone.prompt.bind_media_mut(id, media_ref)?;
    }

    let cloned_arc = Arc::new(RwLock::new(task_clone));
    let agent_id = cloned_arc
        .read()
        .map_err(|_| EvaluationError::TaskLockError)?
        .agent_id
        .clone();
    let agent = workflow
        .agents
        .get(&agent_id)
        .ok_or_else(|| EvaluationError::AgentNotFound(agent_id))?;

    let response = agent
        .execute_task_with_context(&cloned_arc, context)
        .await
        .map_err(|e| EvaluationError::AgentEvaluatorError(e.to_string()))?;

    Ok(response
        .response_value()
        .unwrap_or_else(|| Value::String(response.response_text())))
}

#[cfg(test)]
mod tests {
    use super::build_media_refs;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use potato_head::prompt_types::{MediaKind, MediaSource};
    use scouter_types::agent::media::{EvalMedia, EvalMediaKind, EvalMediaSource};

    #[test]
    fn build_media_refs_url() {
        let media = EvalMedia {
            id: "chart".to_string(),
            kind: EvalMediaKind::Image,
            source: EvalMediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: Some("image/png".to_string()),
            },
        };

        let refs = build_media_refs(&[media]);

        assert_eq!(refs[0].0, "chart");
        assert_eq!(refs[0].1.kind, MediaKind::Image);
        assert_eq!(
            refs[0].1.source,
            MediaSource::Url {
                url: "https://example.com/chart.png".to_string(),
                mime_type: Some("image/png".to_string())
            }
        );
    }

    #[test]
    fn build_media_refs_base64() {
        let data = STANDARD.encode(b"%PDF");
        let media = EvalMedia {
            id: "report".to_string(),
            kind: EvalMediaKind::Document,
            source: EvalMediaSource::Base64 {
                mime_type: "application/pdf".to_string(),
                data: data.clone(),
            },
        };

        let refs = build_media_refs(&[media]);

        assert_eq!(refs[0].0, "report");
        assert_eq!(refs[0].1.kind, MediaKind::Document);
        assert_eq!(
            refs[0].1.source,
            MediaSource::Base64 {
                mime_type: "application/pdf".to_string(),
                data
            }
        );
    }

    #[test]
    fn build_media_refs_preserves_order() {
        let media = ["a", "b", "c"]
            .into_iter()
            .map(|id| EvalMedia {
                id: id.to_string(),
                kind: EvalMediaKind::Image,
                source: EvalMediaSource::Url {
                    url: format!("https://example.com/{id}.png"),
                    mime_type: None,
                },
            })
            .collect::<Vec<_>>();

        let refs = build_media_refs(&media);
        let ids = refs.into_iter().map(|(id, _)| id).collect::<Vec<_>>();

        assert_eq!(ids, vec!["a", "b", "c"]);
    }
}
