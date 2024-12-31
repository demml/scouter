use crate::sql::postgres::PostgresClient;
use anyhow::*;
use scouter::core::drift::base::{RecordType, ServerRecord, ServerRecords};
use scouter::core::drift::custom::types::CustomMetricServerRecord;
use scouter::core::drift::psi::types::PsiServerRecord;
use scouter::core::drift::spc::types::SpcServerRecord;
use scouter::core::observe::observer::ObservabilityMetrics;
use std::result::Result::Ok;
use tracing::error;

pub trait ToDriftRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>>;
    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>>;
    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>>;
    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>>;
}
impl ToDriftRecords for ServerRecords {
    fn to_spc_drift_records(&self) -> Result<Vec<SpcServerRecord>> {
        match self.record_type {
            RecordType::Spc => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Spc {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Observability => Err(anyhow!("Unexpected record type")),
            RecordType::Psi => Err(anyhow!("Unexpected record type")),
            RecordType::Custom => Err(anyhow!("Unexpected record type")),
        }
    }

    fn to_observability_drift_records(&self) -> Result<Vec<ObservabilityMetrics>> {
        match self.record_type {
            RecordType::Spc => Err(anyhow!("Unexpected record type")),
            RecordType::Observability => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Observability {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Psi => Err(anyhow!("Unexpected record type")),
            RecordType::Custom => Err(anyhow!("Unexpected record type")),
        }
    }

    fn to_psi_drift_records(&self) -> Result<Vec<PsiServerRecord>> {
        match self.record_type {
            RecordType::Psi => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Psi {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Observability => Err(anyhow!("Unexpected record type")),
            RecordType::Spc => Err(anyhow!("Unexpected record type")),
            RecordType::Custom => Err(anyhow!("Unexpected record type")),
        }
    }

    fn to_custom_metric_drift_records(&self) -> Result<Vec<CustomMetricServerRecord>> {
        match self.record_type {
            RecordType::Custom => {
                let mut records = Vec::new();
                for record in self.records.iter() {
                    match record {
                        ServerRecord::Custom {
                            record: inner_record,
                        } => {
                            records.push(inner_record.clone());
                        }
                        _ => {
                            error!("Unexpected record type");
                        }
                    }
                }
                Ok(records)
            }
            RecordType::Observability => Err(anyhow!("Unexpected record type")),
            RecordType::Spc => Err(anyhow!("Unexpected record type")),
            RecordType::Psi => Err(anyhow!("Unexpected record type")),
        }
    }
}

pub enum MessageHandler {
    Postgres(PostgresClient),
}

impl MessageHandler {
    pub async fn insert_server_records(&self, records: &ServerRecords) -> Result<()> {
        match self {
            Self::Postgres(client) => {
                match records.record_type {
                    RecordType::Spc => {
                        let records = records.to_spc_drift_records()?;
                        for record in records.iter() {
                            let _ = client.insert_spc_drift_record(record).await.map_err(|e| {
                                error!("Failed to insert drift record: {:?}", e);
                            });
                        }
                    }
                    RecordType::Observability => {
                        let records = records.to_observability_drift_records()?;
                        for record in records.iter() {
                            let _ = client
                                .insert_observability_record(record)
                                .await
                                .map_err(|e| {
                                    error!("Failed to insert observability record: {:?}", e);
                                });
                        }
                    }
                    RecordType::Psi => {
                        let records = records.to_psi_drift_records()?;
                        for record in records.iter() {
                            let _ = client.insert_bin_counts(record).await.map_err(|e| {
                                error!("Failed to insert bin count record: {:?}", e);
                            });
                        }
                    }
                    RecordType::Custom => {
                        let records = records.to_custom_metric_drift_records()?;
                        for record in records.iter() {
                            let _ = client
                                .insert_custom_metric_value(record)
                                .await
                                .map_err(|e| {
                                    error!("Failed to insert bin count record: {:?}", e);
                                });
                        }
                    }
                };
            }
        }

        Ok(())
    }
}
