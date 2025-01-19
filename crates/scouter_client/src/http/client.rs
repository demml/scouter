use pyo3::prelude::*;
use scouter_contracts::DriftRequest;
use scouter_error::{PyScouterError, ScouterError};
use scouter_events::producer::http::{HTTPClient, HTTPConfig, RequestType, Routes};
use scouter_types::DriftType;
use scouter_types::{
    custom::BinnedCustomMetrics, psi::BinnedPsiFeatureMetrics, spc::SpcDriftFeatures,
};
use std::sync::Arc;
use tokio::runtime::Runtime;

#[pyclass]
pub struct ScouterClient {
    client: HTTPClient,
    rt: Arc<Runtime>,
}

#[pymethods]
impl ScouterClient {
    #[new]
    #[pyo3(signature = (config))]
    pub fn new(config: &Bound<'_, PyAny>) -> PyResult<Self> {
        let rt = Arc::new(tokio::runtime::Runtime::new().unwrap());

        if config.is_instance_of::<HTTPConfig>() {
            let config = config.extract::<HTTPConfig>().unwrap();
            let client = rt
                .block_on(async { HTTPClient::new(config).await })
                .unwrap();
            Ok(ScouterClient { client, rt })
        } else {
            Err(PyScouterError::new_err("Invalid config type"))
        }
    }

    pub fn get_drift(&mut self, drift_request: DriftRequest) -> PyResult<()> {
        let query_string = serde_qs::to_string(&drift_request)
            .map_err(|e| PyScouterError::new_err(e.to_string()))?;

        match drift_request.drift_type {
            DriftType::Spc => {
                let _response = self
                    .rt
                    .block_on(async {
                        let response = self
                            .client
                            .request_with_retry(
                                Routes::SpcDrift,
                                RequestType::Get,
                                None,
                                Some(query_string),
                                None,
                            )
                            .await?;
                        let body = response.bytes().await.unwrap().to_vec();
                        let results: SpcDriftFeatures = serde_json::from_slice(&body)
                            .map_err(|e| ScouterError::Error(e.to_string()))?;

                        Ok(results)
                    })
                    .map_err(|e: scouter_error::ScouterError| {
                        PyScouterError::new_err(e.to_string())
                    })?;
                ()
            }
            DriftType::Psi => {
                let _response = self
                    .rt
                    .block_on(async {
                        let response = self
                            .client
                            .request_with_retry(
                                Routes::PsiDrift,
                                RequestType::Get,
                                None,
                                Some(query_string),
                                None,
                            )
                            .await?;
                        let body = response.bytes().await.unwrap().to_vec();
                        let results: BinnedPsiFeatureMetrics = serde_json::from_slice(&body)
                            .map_err(|e| ScouterError::Error(e.to_string()))?;

                        Ok(results)
                    })
                    .map_err(|e: scouter_error::ScouterError| {
                        PyScouterError::new_err(e.to_string())
                    })?;
                ()
            }

            DriftType::Custom => {
                let _response = self
                    .rt
                    .block_on(async {
                        let response = self
                            .client
                            .request_with_retry(
                                Routes::CustomDrift,
                                RequestType::Get,
                                None,
                                Some(query_string),
                                None,
                            )
                            .await?;
                        let body = response.bytes().await.unwrap().to_vec();
                        let results: BinnedCustomMetrics = serde_json::from_slice(&body)
                            .map_err(|e| ScouterError::Error(e.to_string()))?;

                        Ok(results)
                    })
                    .map_err(|e: scouter_error::ScouterError| {
                        PyScouterError::new_err(e.to_string())
                    })?;
                ()
            }
        }

        Ok(())
    }
}
