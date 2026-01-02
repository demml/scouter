//constants

// spc
const INSERT_DRIFT_RECORD: &str = include_str!("scripts/spc/insert_spc_drift_record.sql");
const INSERT_SPC_DRIFT_RECORD_BATCH: &str =
    include_str!("scripts/spc/insert_spc_drift_record_batch.sql");
const GET_SPC_FEATURES: &str = include_str!("scripts/spc/unique_spc_features.sql");
const GET_BINNED_SPC_FEATURE_VALUES: &str =
    include_str!("scripts/spc/binned_spc_feature_values.sql");
const GET_SPC_FEATURE_VALUES: &str = include_str!("scripts/spc/get_spc_feature_values.sql");
const GET_SPC_ENTITIES: &str = include_str!("scripts/spc/get_spc_entities_for_archive.sql");
const GET_SPC_DATA_FOR_ARCHIVE: &str = include_str!("scripts/spc/get_spc_data_for_archive.sql");
const UPDATE_SPC_ENTITIES: &str = include_str!("scripts/spc/update_data_to_archived.sql");

// psi
const INSERT_BIN_COUNTS: &str = include_str!("scripts/psi/insert_bin_counts.sql");
const INSERT_BIN_COUNTS_BATCH: &str = include_str!("scripts/psi/insert_bin_counts_batch.sql");
const GET_BINNED_PSI_FEATURE_BINS: &str =
    include_str!("scripts/psi/binned_psi_feature_bin_proportions.sql");
const GET_FEATURE_BIN_PROPORTIONS: &str =
    include_str!("scripts/psi/get_feature_bin_proportions.sql");
const GET_BIN_COUNT_ENTITIES: &str =
    include_str!("scripts/psi/get_bin_count_entities_for_archive.sql");
const GET_BIN_COUNT_DATA_FOR_ARCHIVE: &str =
    include_str!("scripts/psi/get_bin_count_data_for_archive.sql");
const UPDATE_BIN_COUNT_ENTITIES: &str = include_str!("scripts/psi/update_data_to_archived.sql");

// custom
const GET_BINNED_CUSTOM_METRIC_VALUES: &str =
    include_str!("scripts/custom/binned_custom_metric_values.sql");
const GET_CUSTOM_METRIC_VALUES: &str = include_str!("scripts/custom/get_custom_metric_values.sql");
const INSERT_CUSTOM_METRIC_VALUES: &str =
    include_str!("scripts/custom/insert_custom_metric_values.sql");
const INSERT_CUSTOM_METRIC_VALUES_BATCH: &str =
    include_str!("scripts/custom/insert_custom_metric_values_batch.sql");
const GET_CUSTOM_ENTITIES: &str =
    include_str!("scripts/custom/get_custom_metric_entities_for_archive.sql");
const GET_CUSTOM_DATA_FOR_ARCHIVE: &str =
    include_str!("scripts/custom/get_custom_metric_data_for_archive.sql");
const UPDATE_CUSTOM_ENTITIES: &str = include_str!("scripts/custom/update_data_to_archived.sql");

// genai
// genai insert
const INSERT_GENAI_TASK_RESULTS_BATCH: &str =
    include_str!("scripts/genai/insert_genai_task_results.sql");
const INSERT_GENAI_WORKFLOW_RESULT: &str =
    include_str!("scripts/genai/insert_genai_workflow_result.sql");
const INSERT_GENAI_EVENT_RECORD: &str = include_str!("scripts/genai/insert_genai_event_record.sql");

// genai query
const GET_GENAI_DRIFT_RECORDS: &str = include_str!("scripts/genai/get_genai_event_records.sql");
const GET_GENAI_TASK_VALUES: &str = include_str!("scripts/genai/get_genai_task_values.sql");
const GET_GENAI_WORKFLOW_VALUES: &str = include_str!("scripts/genai/get_genai_workflow_values.sql");
const GET_BINNED_GENAI_METRIC_VALUES: &str =
    include_str!("scripts/genai/binned_genai_workflow_values.sql");

// genai paginated query
const GET_PAGINATED_GENAI_DRIFT_RECORDS: &str =
    include_str!("scripts/genai/get_paginated_genai_event_records.sql");
const UPDATE_GENAI_DRIFT_TASK: &str = include_str!("scripts/genai/update_genai_event_record.sql");

// Archive data
const GET_GENAI_TASK_DATA_FOR_ARCHIVE: &str =
    include_str!("scripts/genai/get_genai_task_data_for_archive.sql");
const GET_GENAI_DRIFT_RECORD_DATA_FOR_ARCHIVE: &str =
    include_str!("scripts/genai/get_genai_event_record_data_for_archive.sql");
const GET_GENAI_WORKFLOW_DATA_FOR_ARCHIVE: &str =
    include_str!("scripts/genai/get_genai_workflow_data_for_archive.sql");
const GET_GENAI_DRIFT_RECORD_ENTITIES: &str =
    include_str!("scripts/genai/get_genai_event_record_entities_for_archive.sql");

// genai update entities
const UPDATE_GENAI_TASK_ENTITIES: &str =
    include_str!("scripts/genai/update_genai_task_to_archived.sql");
const UPDATE_GENAI_WORKFLOW_ENTITIES: &str =
    include_str!("scripts/genai/update_genai_workflow_to_archived.sql");
const UPDATE_GENAI_DRIFT_ENTITIES: &str =
    include_str!("scripts/genai/update_genai_event_record_to_archived.sql");

// observability (experimental)
const GET_BINNED_OBSERVABILITY_METRICS: &str =
    include_str!("scripts/observability/binned_observability_metrics.sql");
const INSERT_OBSERVABILITY_RECORD: &str =
    include_str!("scripts/observability/insert_observability_record.sql");

//profile
const INSERT_DRIFT_PROFILE: &str = include_str!("scripts/profile/insert_drift_profile.sql");
const GET_DRIFT_PROFILE: &str = include_str!("scripts/profile/get_drift_profile.sql");
const UPDATE_DRIFT_PROFILE_RUN_DATES: &str =
    include_str!("scripts/profile/update_drift_profile_run_dates.sql");
const UPDATE_DRIFT_PROFILE_STATUS: &str =
    include_str!("scripts/profile/update_drift_profile_status.sql");
const UPDATE_DRIFT_PROFILE: &str = include_str!("scripts/profile/update_drift_profile.sql");
const DEACTIVATE_DRIFT_PROFILES: &str =
    include_str!("scripts/profile/deactivate_drift_profiles.sql");
const GET_PROFILE_VERSIONS: &str = include_str!("scripts/profile/get_profile_versions.sql");
const LIST_DRIFT_PROFILES: &str = include_str!("scripts/profile/list_drift_profiles.sql");

// alert
const INSERT_DRIFT_ALERT: &str = include_str!("scripts/alert/insert_drift_alert.sql");
const GET_PAGINATED_DRIFT_ALERTS: &str = include_str!("scripts/alert/get_drift_alerts.sql");
const UPDATE_ALERT_STATUS: &str = include_str!("scripts/alert/update_alert_status.sql");

// poll
const GET_DRIFT_TASK: &str = include_str!("scripts/poll/poll_for_drift_task.sql");
const GET_PENDING_GENAI_DRIFT_TASK: &str =
    include_str!("scripts/poll/poll_for_genai_drift_task.sql");

// auth
const INSERT_USER: &str = include_str!("scripts/user/insert_user.sql");
const GET_USER: &str = include_str!("scripts/user/get_user.sql");
const UPDATE_USER: &str = include_str!("scripts/user/update_user.sql");
const GET_USERS: &str = include_str!("scripts/user/get_users.sql");
const LAST_ADMIN: &str = include_str!("scripts/user/last_admin.sql");
const DELETE_USER: &str = include_str!("scripts/user/delete_user.sql");

const INSERT_TRACE_SPAN: &str = include_str!("scripts/trace/insert_span.sql");
const INSERT_TRACE_BAGGAGE: &str = include_str!("scripts/trace/insert_baggage.sql");
const GET_PAGINATED_TRACES: &str = include_str!("scripts/trace/get_paginated_traces.sql");
const GET_TRACE_SPANS: &str = include_str!("scripts/trace/get_trace_spans.sql");
const GET_TRACE_METRICS: &str = include_str!("scripts/trace/get_trace_metrics.sql");
const GET_TRACE_BAGGAGE: &str = include_str!("scripts/trace/get_trace_baggage.sql");

// tags
const INSERT_TAG: &str = include_str!("scripts/tag/insert_tags.sql");
const GET_TAGS: &str = include_str!("scripts/tag/get_tags.sql");
const GET_ENTITY_ID_BY_TAG: &str = include_str!("scripts/tag/get_entity_id_by_tags.sql");

// entity
const GET_ENTITY_ID_FROM_UID: &str = include_str!("scripts/entity/get_id_from_uid.sql");
const GET_ENTITY_ID_FROM_SPACE_NAME_VERSION_DRIFT_TYPE: &str =
    include_str!("scripts/entity/get_id_from_space_name_version_drift_type.sql");

#[allow(dead_code)]
pub enum Queries {
    GetSpcFeatures,
    InsertDriftRecord,
    InsertBinCounts,
    InsertDriftProfile,
    InsertDriftAlert,
    InsertObservabilityRecord,
    GetPaginatedDriftAlerts,
    GetBinnedSpcFeatureValues,
    GetBinnedPsiFeatureBins,
    GetBinnedMetricValues,
    GetBinnedObservabilityMetrics,
    GetSpcFeatureValues,
    GetDriftTask,
    GetDriftProfile,
    UpdateDriftProfileRunDates,
    UpdateDriftProfileStatus,
    DeactivateDriftProfiles,
    UpdateDriftProfile,
    GetFeatureBinProportions,
    GetCustomMetricValues,
    InsertCustomMetricValues,

    InsertCustomMetricValuesBatch,
    InsertSpcDriftRecordBatch,
    InsertBinCountsBatch,

    // archive
    // entities
    GetBinCountEntities,
    GetCustomEntities,
    GetSpcEntities,

    // data
    GetBinCountDataForArchive,
    GetCustomDataForArchive,
    GetSpcDataForArchive,

    // update
    UpdateBinCountEntities,
    UpdateCustomEntities,
    UpdateSpcEntities,

    // user
    InsertUser,
    GetUser,
    UpdateUser,
    GetUsers,
    LastAdmin,
    DeleteUser,
    UpdateAlertStatus,

    // genai -binned
    GetGenAIWorkflowBinnedMetrics,

    // genai - query
    GetGenAIEventRecords,
    GetPendingGenAIEventTask,
    GetPaginatedGenAIEventRecords,

    // genai - insert
    InsertGenAITaskResultsBatch,
    InsertGenAIWorkflowResult,
    InsertGenAIEventRecord,

    // genai - update
    UpdateGenAIEventTask,

    // Genai - archive
    GetGenAIEventRecordEntitiesForArchive,
    GetGenAIEventRecordDataForArchive,
    GetGenAITaskResultDataForArchive,
    GetGenAIWorkflowResultDataForArchive,

    UpdateGenAIEventEntities,
    UpdateGenAITaskEntities,
    UpdateGenAIWorkflowEntities,

    // profile
    GetProfileVersions,
    ListDriftProfiles,

    //trace
    InsertTraceSpan,
    InsertTraceBaggage,
    GetPaginatedTraces,
    GetTraceSpans,
    GetTraceMetrics,
    GetTraceBaggage,

    // tags
    InsertTag,
    GetTags,
    GetEntityIdByTags,

    // entity
    GetEntityIdFromUid,
    GetEntityIdFromSpaceNameVersionDriftType,
}

impl Queries {
    // TODO: shouldn't we just return the string directly? Not sure if that's true for all db operations, I'm
    // just noticing it in the few that im working on. (user related queries)
    pub fn get_query(&self) -> &'static str {
        match self {
            // load sql file from scripts/insert.sql
            Queries::GetSpcFeatures => GET_SPC_FEATURES,
            Queries::InsertDriftRecord => INSERT_DRIFT_RECORD,
            Queries::GetBinnedSpcFeatureValues => GET_BINNED_SPC_FEATURE_VALUES,
            Queries::GetBinnedPsiFeatureBins => GET_BINNED_PSI_FEATURE_BINS,
            Queries::GetBinnedMetricValues => GET_BINNED_CUSTOM_METRIC_VALUES,
            Queries::GetBinnedObservabilityMetrics => GET_BINNED_OBSERVABILITY_METRICS,
            Queries::GetSpcFeatureValues => GET_SPC_FEATURE_VALUES,
            Queries::InsertDriftProfile => INSERT_DRIFT_PROFILE,
            Queries::InsertDriftAlert => INSERT_DRIFT_ALERT,
            Queries::InsertObservabilityRecord => INSERT_OBSERVABILITY_RECORD,
            Queries::GetPaginatedDriftAlerts => GET_PAGINATED_DRIFT_ALERTS,
            Queries::GetDriftTask => GET_DRIFT_TASK,
            Queries::UpdateDriftProfileRunDates => UPDATE_DRIFT_PROFILE_RUN_DATES,
            Queries::UpdateDriftProfileStatus => UPDATE_DRIFT_PROFILE_STATUS,
            Queries::UpdateDriftProfile => UPDATE_DRIFT_PROFILE,
            Queries::DeactivateDriftProfiles => DEACTIVATE_DRIFT_PROFILES,
            Queries::GetDriftProfile => GET_DRIFT_PROFILE,
            Queries::GetFeatureBinProportions => GET_FEATURE_BIN_PROPORTIONS,
            Queries::InsertBinCounts => INSERT_BIN_COUNTS,
            Queries::GetCustomMetricValues => GET_CUSTOM_METRIC_VALUES,
            Queries::InsertCustomMetricValues => INSERT_CUSTOM_METRIC_VALUES,
            Queries::GetBinCountEntities => GET_BIN_COUNT_ENTITIES,
            Queries::GetCustomEntities => GET_CUSTOM_ENTITIES,
            Queries::GetSpcEntities => GET_SPC_ENTITIES,
            Queries::GetBinCountDataForArchive => GET_BIN_COUNT_DATA_FOR_ARCHIVE,
            Queries::GetCustomDataForArchive => GET_CUSTOM_DATA_FOR_ARCHIVE,
            Queries::GetSpcDataForArchive => GET_SPC_DATA_FOR_ARCHIVE,
            Queries::UpdateBinCountEntities => UPDATE_BIN_COUNT_ENTITIES,
            Queries::UpdateCustomEntities => UPDATE_CUSTOM_ENTITIES,
            Queries::UpdateSpcEntities => UPDATE_SPC_ENTITIES,
            Queries::GetProfileVersions => GET_PROFILE_VERSIONS,
            Queries::ListDriftProfiles => LIST_DRIFT_PROFILES,
            Queries::InsertUser => INSERT_USER,
            Queries::GetUser => GET_USER,
            Queries::UpdateUser => UPDATE_USER,
            Queries::GetUsers => GET_USERS,
            Queries::LastAdmin => LAST_ADMIN,
            Queries::DeleteUser => DELETE_USER,
            Queries::UpdateAlertStatus => UPDATE_ALERT_STATUS,

            //genai
            Queries::GetGenAIMetricValues => GET_GENAI_METRIC_VALUES,
            Queries::GetBinnedMetrics => GET_BINNED_GENAI_METRIC_VALUES,
            Queries::InsertGenAITaskResultsBatch => INSERT_GENAI_TASK_RESULTS_BATCH,
            Queries::InsertGenAIWorkflowResult => INSERT_GENAI_WORKFLOW_RESULT,
            Queries::InsertGenAIEventRecord => INSERT_GENAI_EVENT_RECORD,

            Queries::GetGenAIDriftRecords => GET_GENAI_DRIFT_RECORDS,
            Queries::GetPaginatedGenAIDriftRecords => GET_PAGINATED_GENAI_DRIFT_RECORDS,
            Queries::GetPendingGenAIDriftTask => GET_PENDING_GENAI_DRIFT_TASK,
            Queries::GetGenAIDriftRecordEntitiesForArchive => GET_GENAI_DRIFT_RECORD_ENTITIES,
            Queries::GetGenAIDriftRecordDataForArchive => GET_GENAI_DRIFT_RECORD_DATA_FOR_ARCHIVE,
            Queries::GetGenAIMetricDataForArchive => GET_GENAI_METRIC_DATA_FOR_ARCHIVE,
            Queries::UpdateGenAIDriftEntities => UPDATE_GENAI_DRIFT_ENTITIES,

            Queries::InsertCustomMetricValuesBatch => INSERT_CUSTOM_METRIC_VALUES_BATCH,
            Queries::InsertSpcDriftRecordBatch => INSERT_SPC_DRIFT_RECORD_BATCH,
            Queries::InsertBinCountsBatch => INSERT_BIN_COUNTS_BATCH,
            Queries::UpdateGenAIDriftTask => UPDATE_GENAI_DRIFT_TASK,
            // trace
            Queries::InsertTraceSpan => INSERT_TRACE_SPAN,
            Queries::InsertTraceBaggage => INSERT_TRACE_BAGGAGE,
            Queries::GetPaginatedTraces => GET_PAGINATED_TRACES,
            Queries::GetTraceSpans => GET_TRACE_SPANS,
            Queries::GetTraceMetrics => GET_TRACE_METRICS,
            Queries::GetTraceBaggage => GET_TRACE_BAGGAGE,

            // tags
            Queries::InsertTag => INSERT_TAG,
            Queries::GetTags => GET_TAGS,
            Queries::GetEntityIdByTags => GET_ENTITY_ID_BY_TAG,
            // entity
            Queries::GetEntityIdFromUid => GET_ENTITY_ID_FROM_UID,
            Queries::GetEntityIdFromSpaceNameVersionDriftType => {
                GET_ENTITY_ID_FROM_SPACE_NAME_VERSION_DRIFT_TYPE
            }
        }
    }
}
