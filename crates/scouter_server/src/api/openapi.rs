use crate::api::state::AppState;
use axum::Router;
use std::sync::Arc;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
            components.add_security_scheme(
                "bearer_token",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Scouter API",
        version = "1.0.0",
        description = "ML observability and drift monitoring platform"
    ),
    paths(
        // health
        crate::api::routes::health::route::health_check,
        // auth
        crate::api::routes::auth::route::api_login_handler,
        crate::api::routes::auth::route::api_refresh_token_handler,
        crate::api::routes::auth::route::validate_jwt_token,
        // profile
        crate::api::routes::profile::route::insert_drift_profile,
        crate::api::routes::profile::route::update_drift_profile,
        crate::api::routes::profile::route::get_profile,
        crate::api::routes::profile::route::list_profiles,
        crate::api::routes::profile::route::update_drift_profile_status,
        // drift
        crate::api::routes::drift::route::get_spc_drift,
        crate::api::routes::drift::route::get_psi_drift,
        crate::api::routes::drift::route::get_custom_drift,
        crate::api::routes::drift::route::get_agent_task_metrics,
        crate::api::routes::drift::route::get_agent_workflow_metrics,
        crate::api::routes::drift::route::insert_drift,
        // alerts
        crate::api::routes::alerts::route::drift_alerts,
        crate::api::routes::alerts::route::update_alert_status,
        // users
        crate::api::routes::user::route::create_user,
        crate::api::routes::user::route::get_user,
        crate::api::routes::user::route::list_users,
        crate::api::routes::user::route::update_user,
        crate::api::routes::user::route::delete_user,
        // observability
        crate::api::routes::observability::route::get_observability_metrics,
        // tags
        crate::api::routes::tags::route::get_tags,
        crate::api::routes::tags::route::insert_tags,
        crate::api::routes::tags::route::entity_id_from_tags,
        // trace
        crate::api::routes::trace::route::get_trace_baggage,
        crate::api::routes::trace::route::paginated_traces,
        crate::api::routes::trace::route::get_trace_spans_by_id,
        crate::api::routes::trace::route::get_trace_spans,
        crate::api::routes::trace::route::query_trace_spans_from_tags,
        crate::api::routes::trace::route::trace_metrics,
        crate::api::routes::trace::route::query_spans_from_filters,
        crate::api::routes::trace::route::v1_otel_traces,
        crate::api::routes::trace::route::debug_recent_traces,
        // message
        crate::api::routes::message::route::insert_message,
        // agent
        crate::api::routes::agent::route::query_agent_eval_records,
        crate::api::routes::agent::route::query_agent_eval_workflow,
        crate::api::routes::agent::route::get_agent_tasks,
        // dataset
        crate::api::routes::dataset::route::register_dataset,
        crate::api::routes::dataset::route::insert_batch,
        crate::api::routes::dataset::route::query_dataset,
        crate::api::routes::dataset::route::list_datasets_handler,
        crate::api::routes::dataset::route::get_dataset_info,
        crate::api::routes::dataset::route::list_catalogs,
        crate::api::routes::dataset::route::list_schemas,
        crate::api::routes::dataset::route::list_tables,
        crate::api::routes::dataset::route::get_table_detail,
        crate::api::routes::dataset::route::preview_table,
        crate::api::routes::dataset::route::execute_query,
        crate::api::routes::dataset::route::cancel_query,
        crate::api::routes::dataset::route::explain_query,
        // service map
        crate::api::routes::service_map::route::get_service_graph,
        // eval scenarios
        crate::api::routes::eval_scenarios::route::get_eval_scenarios,
        // capabilities
        crate::api::routes::capabilities::route::capabilities,
        // docs
        crate::api::routes::docs::route::list_docs,
        crate::api::routes::docs::route::search_docs,
        crate::api::routes::docs::route::get_doc,
    ),
    components(schemas(
        // scouter_types contracts
        scouter_types::contracts::ScouterServerError,
        scouter_types::contracts::ScouterResponse,
        scouter_types::contracts::ListProfilesRequest,
        scouter_types::contracts::ListedProfile,
        scouter_types::contracts::GetProfileRequest,
        scouter_types::contracts::ProfileRequest,
        scouter_types::contracts::ProfileStatusRequest,
        scouter_types::contracts::RegisteredProfileResponse,
        scouter_types::contracts::DriftAlertPaginationRequest,
        scouter_types::contracts::DriftAlertPaginationResponse,
        scouter_types::contracts::RecordCursor,
        scouter_types::contracts::UpdateAlertStatus,
        scouter_types::contracts::UpdateAlertResponse,
        scouter_types::contracts::AgentEvalTaskRequest,
        scouter_types::contracts::AgentEvalTaskResponse,
        scouter_types::contracts::EvalRecordPaginationRequest,
        scouter_types::contracts::EvalRecordPaginationResponse,
        scouter_types::contracts::AgentEvalWorkflowPaginationResponse,
        scouter_types::contracts::TagsRequest,
        scouter_types::contracts::InsertTagsRequest,
        scouter_types::contracts::EntityIdTagsRequest,
        scouter_types::contracts::TagsResponse,
        scouter_types::contracts::EntityIdTagsResponse,
        scouter_types::contracts::SpansFromTagsRequest,
        scouter_types::contracts::TraceMetricsRequest,
        scouter_types::contracts::TraceBaggageResponse,
        scouter_types::contracts::TracePaginationResponse,
        scouter_types::contracts::TraceSpansResponse,
        scouter_types::contracts::TraceMetricsResponse,
        scouter_types::contracts::TraceReceivedResponse,
        scouter_types::contracts::TraceCursor,
        scouter_types::contracts::BinnedMetrics,
        scouter_types::contracts::BinnedMetric,
        scouter_types::contracts::BinnedMetricStats,
        scouter_types::JwtToken,
        scouter_types::trace::sql::TraceFilters,
        scouter_types::psi::BinnedPsiFeatureMetrics,
        scouter_types::spc::SpcDriftFeatures,
        // local server types
        crate::api::routes::health::route::Alive,
        crate::api::routes::auth::schema::Authenticated,
        crate::api::routes::auth::schema::AuthError,
        crate::api::routes::user::schema::CreateUserRequest,
        crate::api::routes::user::schema::UpdateUserRequest,
        crate::api::routes::user::schema::UserResponse,
        crate::api::routes::user::schema::UserListResponse,
        crate::api::routes::user::schema::CreateUserResponse,
        // capabilities
        crate::api::routes::capabilities::route::CapabilitiesResponse,
        crate::api::routes::capabilities::route::FeaturesInfo,
        crate::api::routes::capabilities::route::EndpointsInfo,
        crate::api::routes::capabilities::route::AuthInfo,
        // docs
        crate::api::routes::docs::route::DocListResponse,
        crate::api::routes::docs::route::DocSummary,
        crate::api::routes::docs::route::DocResponse,
        crate::api::routes::docs::route::DocSearchResponse,
        crate::api::routes::docs::route::DocSearchResult,
    )),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "auth", description = "Authentication endpoints"),
        (name = "drift", description = "Drift detection endpoints"),
        (name = "profile", description = "Drift profile management endpoints"),
        (name = "alerts", description = "Alert management endpoints"),
        (name = "users", description = "User management endpoints"),
        (name = "tags", description = "Tag management endpoints"),
        (name = "traces", description = "Distributed tracing endpoints"),
        (name = "messages", description = "Message ingestion endpoints"),
        (name = "observability", description = "Observability metrics endpoints"),
        (name = "agent", description = "Agent evaluation endpoints"),
        (name = "datasets", description = "Dataset management endpoints"),
        (name = "eval", description = "Evaluation scenario endpoints"),
        (name = "capabilities", description = "Server capabilities and discovery"),
        (name = "docs", description = "Embedded documentation and search"),
        (name = "service_map", description = "Service dependency map endpoints"),
    ),
    modifiers(&SecurityAddon)
)]
pub struct ApiDoc;

pub fn openapi_router() -> Router<Arc<AppState>> {
    SwaggerUi::new("/scouter/api/v1/docs/ui")
        .url("/scouter/api/v1/openapi.json", ApiDoc::openapi())
        .into()
}
