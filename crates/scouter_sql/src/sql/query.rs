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

// llm
const GET_LLM_METRIC_VALUES: &str = include_str!("scripts/llm/get_llm_metric_values.sql");
const GET_BINNED_LLM_METRIC_VALUES: &str = include_str!("scripts/llm/binned_llm_metric_values.sql");
const INSERT_LLM_METRIC_VALUES_BATCH: &str =
    include_str!("scripts/llm/insert_llm_metric_values.sql");
const INSERT_LLM_DRIFT_RECORD: &str = include_str!("scripts/llm/insert_llm_drift_record.sql");
const GET_LLM_ENTITIES: &str = include_str!("scripts/llm/get_llm_metric_entities_for_archive.sql");
const GET_LLM_DATA_FOR_ARCHIVE: &str =
    include_str!("scripts/llm/get_llm_metric_data_for_archive.sql");
const UPDATE_LLM_ENTITIES: &str = include_str!("scripts/llm/update_data_to_archived.sql");
const GET_LLM_DRIFT_RECORDS: &str = include_str!("scripts/llm/get_llm_drift_records.sql");

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

// alert
const INSERT_DRIFT_ALERT: &str = include_str!("scripts/alert/insert_drift_alert.sql");
const GET_DRIFT_ALERTS: &str = include_str!("scripts/alert/get_drift_alerts.sql");
const UPDATE_ALERT_STATUS: &str = include_str!("scripts/alert/update_alert_status.sql");

// poll
const GET_DRIFT_TASK: &str = include_str!("scripts/poll/poll_for_drift_task.sql");

// auth
const INSERT_USER: &str = include_str!("scripts/user/insert_user.sql");
const GET_USER: &str = include_str!("scripts/user/get_user.sql");
const UPDATE_USER: &str = include_str!("scripts/user/update_user.sql");
const GET_USERS: &str = include_str!("scripts/user/get_users.sql");
const LAST_ADMIN: &str = include_str!("scripts/user/last_admin.sql");
const DELETE_USER: &str = include_str!("scripts/user/delete_user.sql");

#[allow(dead_code)]
pub enum Queries {
    GetSpcFeatures,
    InsertDriftRecord,
    InsertBinCounts,
    InsertDriftProfile,
    InsertDriftAlert,
    InsertObservabilityRecord,
    GetDriftAlerts,
    GetBinnedSpcFeatureValues,
    GetBinnedPsiFeatureBins,
    GetBinnedCustomMetricValues,
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

    // llm
    GetLLMMetricValues,
    GetLLMDriftRecords,
    GetBinnedLLMMetrics,
    InsertLLMMetricValuesBatch,
    InsertLLMDriftRecord,
    GetLLMEntities,
    GetLLMDataForArchive,
    UpdateLLMEntities,
}

impl Queries {
    // TODO: shouldn't we just return the string directly? Not sure if that's true for all db operations, I'm
    // just noticing it in the few that im working on. (user related queries)
    pub fn get_query(&self) -> SqlQuery {
        match self {
            // load sql file from scripts/insert.sql
            Queries::GetSpcFeatures => SqlQuery::new(GET_SPC_FEATURES),
            Queries::InsertDriftRecord => SqlQuery::new(INSERT_DRIFT_RECORD),
            Queries::GetBinnedSpcFeatureValues => SqlQuery::new(GET_BINNED_SPC_FEATURE_VALUES),
            Queries::GetBinnedPsiFeatureBins => SqlQuery::new(GET_BINNED_PSI_FEATURE_BINS),
            Queries::GetBinnedCustomMetricValues => SqlQuery::new(GET_BINNED_CUSTOM_METRIC_VALUES),
            Queries::GetBinnedObservabilityMetrics => {
                SqlQuery::new(GET_BINNED_OBSERVABILITY_METRICS)
            }
            Queries::GetSpcFeatureValues => SqlQuery::new(GET_SPC_FEATURE_VALUES),
            Queries::InsertDriftProfile => SqlQuery::new(INSERT_DRIFT_PROFILE),
            Queries::InsertDriftAlert => SqlQuery::new(INSERT_DRIFT_ALERT),
            Queries::InsertObservabilityRecord => SqlQuery::new(INSERT_OBSERVABILITY_RECORD),
            Queries::GetDriftAlerts => SqlQuery::new(GET_DRIFT_ALERTS),
            Queries::GetDriftTask => SqlQuery::new(GET_DRIFT_TASK),
            Queries::UpdateDriftProfileRunDates => SqlQuery::new(UPDATE_DRIFT_PROFILE_RUN_DATES),
            Queries::UpdateDriftProfileStatus => SqlQuery::new(UPDATE_DRIFT_PROFILE_STATUS),
            Queries::UpdateDriftProfile => SqlQuery::new(UPDATE_DRIFT_PROFILE),
            Queries::DeactivateDriftProfiles => SqlQuery::new(DEACTIVATE_DRIFT_PROFILES),
            Queries::GetDriftProfile => SqlQuery::new(GET_DRIFT_PROFILE),
            Queries::GetFeatureBinProportions => SqlQuery::new(GET_FEATURE_BIN_PROPORTIONS),
            Queries::InsertBinCounts => SqlQuery::new(INSERT_BIN_COUNTS),
            Queries::GetCustomMetricValues => SqlQuery::new(GET_CUSTOM_METRIC_VALUES),
            Queries::InsertCustomMetricValues => SqlQuery::new(INSERT_CUSTOM_METRIC_VALUES),
            Queries::GetBinCountEntities => SqlQuery::new(GET_BIN_COUNT_ENTITIES),
            Queries::GetCustomEntities => SqlQuery::new(GET_CUSTOM_ENTITIES),
            Queries::GetSpcEntities => SqlQuery::new(GET_SPC_ENTITIES),
            Queries::GetBinCountDataForArchive => SqlQuery::new(GET_BIN_COUNT_DATA_FOR_ARCHIVE),
            Queries::GetCustomDataForArchive => SqlQuery::new(GET_CUSTOM_DATA_FOR_ARCHIVE),
            Queries::GetSpcDataForArchive => SqlQuery::new(GET_SPC_DATA_FOR_ARCHIVE),
            Queries::UpdateBinCountEntities => SqlQuery::new(UPDATE_BIN_COUNT_ENTITIES),
            Queries::UpdateCustomEntities => SqlQuery::new(UPDATE_CUSTOM_ENTITIES),
            Queries::UpdateSpcEntities => SqlQuery::new(UPDATE_SPC_ENTITIES),

            Queries::InsertUser => SqlQuery::new(INSERT_USER),
            Queries::GetUser => SqlQuery::new(GET_USER),
            Queries::UpdateUser => SqlQuery::new(UPDATE_USER),
            Queries::GetUsers => SqlQuery::new(GET_USERS),
            Queries::LastAdmin => SqlQuery::new(LAST_ADMIN),
            Queries::DeleteUser => SqlQuery::new(DELETE_USER),
            Queries::UpdateAlertStatus => SqlQuery::new(UPDATE_ALERT_STATUS),

            //llm
            Queries::GetLLMMetricValues => SqlQuery::new(GET_LLM_METRIC_VALUES),
            Queries::GetBinnedLLMMetrics => SqlQuery::new(GET_BINNED_LLM_METRIC_VALUES),
            Queries::InsertLLMMetricValuesBatch => SqlQuery::new(INSERT_LLM_METRIC_VALUES_BATCH),
            Queries::InsertLLMDriftRecord => SqlQuery::new(INSERT_LLM_DRIFT_RECORD),
            Queries::GetLLMEntities => SqlQuery::new(GET_LLM_ENTITIES),
            Queries::GetLLMDataForArchive => SqlQuery::new(GET_LLM_DATA_FOR_ARCHIVE),
            Queries::UpdateLLMEntities => SqlQuery::new(UPDATE_LLM_ENTITIES),
            Queries::GetLLMDriftRecords => SqlQuery::new(GET_LLM_DRIFT_RECORDS),

            Queries::InsertCustomMetricValuesBatch => {
                SqlQuery::new(INSERT_CUSTOM_METRIC_VALUES_BATCH)
            }
            Queries::InsertSpcDriftRecordBatch => SqlQuery::new(INSERT_SPC_DRIFT_RECORD_BATCH),
            Queries::InsertBinCountsBatch => SqlQuery::new(INSERT_BIN_COUNTS_BATCH),
        }
    }
}

pub struct SqlQuery {
    pub sql: String,
}

impl SqlQuery {
    fn new(sql: &str) -> Self {
        Self {
            sql: sql.to_string(),
        }
    }
}
