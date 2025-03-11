use scouter_error::DispatchError;
use scouter_types::{
    AlertDispatchConfig, AlertDispatchType, DispatchAlertDescription, DispatchDriftConfig,
    DriftArgs, OpsGenieDispatchConfig, SlackDispatchConfig,
};
use serde_json::{json, Value};
use std::result::Result;
use std::{collections::HashMap, env};
use tracing::error;

trait DispatchHelpers {
    fn construct_alert_description<T: DispatchAlertDescription>(
        &self,
        feature_alerts: &T,
    ) -> String;
}
pub trait Dispatch {
    fn process_alerts<T: DispatchAlertDescription + std::marker::Sync>(
        &self,
        feature_alerts: &T,
    ) -> impl std::future::Future<Output = Result<(), DispatchError>> + Send;
}
pub trait HttpAlertWrapper {
    fn api_url(&self) -> &str;
    fn header_auth_value(&self) -> &str;
    fn construct_alert_body(&self, alert_description: &str) -> Value;
}

#[derive(Debug)]
pub struct OpsGenieAlerter {
    header_auth_value: String,
    api_url: String,
    team_name: String,
    name: String,
    repository: String,
    version: String,
}

impl OpsGenieAlerter {
    /// Create a new OpsGenieAlerter
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the model
    /// * `repository` - Repository of the model
    /// * `version` - Version of the model
    /// * `dispatch_config` - OpsGenieAlerter dispatch configuration
    ///
    pub fn new(
        name: &str,
        repository: &str,
        version: &str,
        dispatch_config: &OpsGenieDispatchConfig,
    ) -> Result<Self, DispatchError> {
        let api_key = env::var("OPSGENIE_API_KEY")
            .map_err(|_| DispatchError::OpsGenieError("OPSGENIE_API_KEY is not set".to_string()))?;

        let api_url = env::var("OPSGENIE_API_URL")
            .map_err(|_| DispatchError::OpsGenieError("OPSGENIE_API_URL is not set".to_string()))?;

        let team_name = dispatch_config.team.clone();

        Ok(Self {
            header_auth_value: format!("GenieKey {}", api_key),
            api_url,
            team_name,
            name: name.to_string(),
            repository: repository.to_string(),
            version: version.to_string(),
        })
    }
}

impl HttpAlertWrapper for OpsGenieAlerter {
    fn api_url(&self) -> &str {
        &self.api_url
    }

    fn header_auth_value(&self) -> &str {
        &self.header_auth_value
    }

    fn construct_alert_body(&self, alert_description: &str) -> Value {
        let mut mapping: HashMap<&str, Value> = HashMap::new();
        mapping.insert(
            "message",
            format!(
                "Model drift detected for {}/{}/{}",
                self.repository, self.name, self.version
            )
            .into(),
        );
        mapping.insert("description", alert_description.to_string().into());
        mapping.insert(
            "responders",
            json!([{"name": self.team_name, "type": "team"}]),
        );
        mapping.insert(
            "visibleTo",
            json!([{"name": self.team_name, "type": "team"}]),
        );

        mapping.insert("tags", json!(["Model Drift", "Scouter"]));
        mapping.insert("priority", "P1".into());

        json!(mapping)
    }
}
impl DispatchHelpers for OpsGenieAlerter {
    fn construct_alert_description<T: DispatchAlertDescription>(
        &self,
        feature_alerts: &T,
    ) -> String {
        feature_alerts.create_alert_description(AlertDispatchType::OpsGenie)
    }
}

#[derive(Debug)]
pub struct SlackAlerter {
    header_auth_value: String,
    api_url: String,
    name: String,
    repository: String,
    version: String,
    channel: String,
}

impl SlackAlerter {
    /// Create a new SlackAlerter
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the model
    /// * `repository` - Repository of the model
    /// * `version` - Version of the model
    /// * `dispatch_config` - slack dispatch configuration
    ///
    pub fn new(
        name: &str,
        repository: &str,
        version: &str,
        dispatch_config: &SlackDispatchConfig,
    ) -> Result<Self, DispatchError> {
        let app_token = env::var("SLACK_APP_TOKEN")
            .map_err(|_| DispatchError::SlackError("SLACK_APP_TOKEN not set".to_string()))?;

        let api_url = env::var("SLACK_API_URL")
            .map_err(|_| DispatchError::SlackError("SLACK_API_URL not set".to_string()))?;

        let slack_channel = dispatch_config.channel.clone();

        Ok(Self {
            header_auth_value: format!("Bearer {}", app_token),
            api_url: format!("{}/chat.postMessage", api_url),
            name: name.to_string(),
            repository: repository.to_string(),
            version: version.to_string(),
            channel: slack_channel,
        })
    }
}

impl HttpAlertWrapper for SlackAlerter {
    fn api_url(&self) -> &str {
        &self.api_url
    }

    fn header_auth_value(&self) -> &str {
        &self.header_auth_value
    }

    fn construct_alert_body(&self, alert_description: &str) -> Value {
        json!({
            "channel": self.channel,
            "blocks": [
                {
                    "type": "header",
                    "text": {
                      "type": "plain_text",
                      "text": ":rotating_light: Drift Detected :rotating_light:",
                      "emoji": true
                    }
                },
                {
                    "type": "section",
                    "text": {
                      "type": "mrkdwn",
                      "text": format!("*Name*: {} *Repository*: {} *Version*: {}", self.name, self.repository, self.version),
                    }
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": alert_description
                    },

                }
            ]
        })
    }
}

impl DispatchHelpers for SlackAlerter {
    fn construct_alert_description<T: DispatchAlertDescription>(
        &self,
        feature_alerts: &T,
    ) -> String {
        feature_alerts.create_alert_description(AlertDispatchType::Slack)
    }
}

#[derive(Debug)]
pub struct HttpAlertDispatcher<T: HttpAlertWrapper> {
    http_client: reqwest::Client,
    alerter: T,
}

impl<T: HttpAlertWrapper> HttpAlertDispatcher<T> {
    pub fn new(alerter: T) -> Self {
        Self {
            http_client: reqwest::Client::new(),
            alerter,
        }
    }

    async fn send_alerts(&self, body: Value) -> Result<(), DispatchError> {
        let response = self
            .http_client
            .post(self.alerter.api_url())
            .header("Authorization", self.alerter.header_auth_value())
            .json(&body)
            .send()
            .await
            .map_err(|e| DispatchError::HttpError(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            let text = response
                .text()
                .await
                .unwrap_or("Failed to parse response".to_string());
            error!("Failed to send alert: {}. Continuing", text);
            Ok(())
        }
    }
}

impl<T: HttpAlertWrapper + DispatchHelpers + std::marker::Sync> Dispatch
    for HttpAlertDispatcher<T>
{
    async fn process_alerts<J: DispatchAlertDescription>(
        &self,
        feature_alerts: &J,
    ) -> Result<(), DispatchError> {
        let alert_description = self.alerter.construct_alert_description(feature_alerts);

        let alert_body = self.alerter.construct_alert_body(&alert_description);

        self.send_alerts(alert_body)
            .await
            .map_err(|e| DispatchError::HttpError(format!("Failed to send alerts: {}", e)))?;

        Ok(())
    }
}

#[derive(Debug)]
pub struct ConsoleAlertDispatcher {
    name: String,
    repository: String,
    version: String,
}

impl ConsoleAlertDispatcher {
    pub fn new(name: &str, repository: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            repository: repository.to_string(),
            version: version.to_string(),
        }
    }
}

impl Dispatch for ConsoleAlertDispatcher {
    async fn process_alerts<T: DispatchAlertDescription>(
        &self,
        feature_alerts: &T,
    ) -> Result<(), DispatchError> {
        let alert_description = self.construct_alert_description(feature_alerts);
        if !alert_description.is_empty() {
            let msg1 = "Drift detected for";
            let msg2 = format!("{}/{}/{}!", self.repository, self.name, self.version);
            let mut body = format!("\n{} {} \n", msg1, msg2);
            body.push_str(&alert_description);

            println!("{}", body);
        }
        Ok(())
    }
}

impl DispatchHelpers for ConsoleAlertDispatcher {
    fn construct_alert_description<T: DispatchAlertDescription>(
        &self,
        feature_alerts: &T,
    ) -> String {
        feature_alerts.create_alert_description(AlertDispatchType::Console)
    }
}

#[derive(Debug)]
pub enum AlertDispatcher {
    Console(ConsoleAlertDispatcher),
    OpsGenie(HttpAlertDispatcher<OpsGenieAlerter>),
    Slack(HttpAlertDispatcher<SlackAlerter>),
}

impl AlertDispatcher {
    // process alerts can be called asynchronously
    pub async fn process_alerts<T: DispatchAlertDescription + std::marker::Sync>(
        &self,
        feature_alerts: &T,
    ) -> Result<(), DispatchError> {
        match self {
            AlertDispatcher::Console(dispatcher) => dispatcher
                .process_alerts(feature_alerts)
                .await
                .map_err(|e| DispatchError::AlertProcessError(e.to_string())),
            AlertDispatcher::OpsGenie(dispatcher) => dispatcher
                .process_alerts(feature_alerts)
                .await
                .map_err(|e| DispatchError::AlertProcessError(e.to_string())),
            AlertDispatcher::Slack(dispatcher) => dispatcher
                .process_alerts(feature_alerts)
                .await
                .map_err(|e| DispatchError::AlertProcessError(e.to_string())),
        }
    }

    // create a new alert dispatcher based on the configuration
    //
    // # Arguments
    //
    // * `config` - DriftConfig (this is an enum wrapper for the different drift configurations)
    pub fn new<T: DispatchDriftConfig>(config: &T) -> Result<Self, DispatchError> {
        let args: DriftArgs = config.get_drift_args();

        let result = match args.dispatch_config {
            AlertDispatchConfig::Slack(config) => {
                SlackAlerter::new(&args.name, &args.repository, &args.version, &config)
                    .map(|alerter| AlertDispatcher::Slack(HttpAlertDispatcher::new(alerter)))
            }
            AlertDispatchConfig::OpsGenie(config) => {
                OpsGenieAlerter::new(&args.name, &args.repository, &args.version, &config)
                    .map(|alerter| AlertDispatcher::OpsGenie(HttpAlertDispatcher::new(alerter)))
            }
            AlertDispatchConfig::Console => Ok(AlertDispatcher::Console(
                ConsoleAlertDispatcher::new(&args.name, &args.repository, &args.version),
            )),
        };

        match result {
            Ok(dispatcher) => Ok(dispatcher),
            Err(e) => {
                error!("Failed to create Alerter: {:?}", e);
                Ok(AlertDispatcher::Console(ConsoleAlertDispatcher::new(
                    &args.name,
                    &args.repository,
                    &args.version,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scouter_types::spc::{
        AlertZone, SpcAlert, SpcAlertConfig, SpcAlertType, SpcDriftConfig, SpcFeatureAlert,
        SpcFeatureAlerts,
    };

    use std::collections::HashMap;
    use std::env;

    fn test_features_map() -> HashMap<String, SpcFeatureAlert> {
        let mut features: HashMap<String, SpcFeatureAlert> = HashMap::new();

        features.insert(
            "test_feature_1".to_string(),
            SpcFeatureAlert {
                feature: "test_feature_1".to_string(),
                alerts: vec![SpcAlert {
                    zone: AlertZone::Zone4,
                    kind: SpcAlertType::OutOfBounds,
                }]
                .into_iter()
                .collect(),
            },
        );
        features.insert(
            "test_feature_2".to_string(),
            SpcFeatureAlert {
                feature: "test_feature_2".to_string(),
                alerts: vec![SpcAlert {
                    zone: AlertZone::Zone1,
                    kind: SpcAlertType::Consecutive,
                }]
                .into_iter()
                .collect(),
            },
        );
        features
    }
    #[test]
    fn test_construct_opsgenie_alert_description() {
        unsafe {
            env::set_var("OPSGENIE_API_URL", "api_url");
            env::set_var("OPSGENIE_API_KEY", "api_key");
        }
        let features = test_features_map();
        let alerter = OpsGenieAlerter::new(
            "name",
            "repository",
            "1.0.0",
            &OpsGenieDispatchConfig {
                team: "test-team".to_string(),
            },
        )
        .unwrap();
        let alert_description = alerter.construct_alert_description(&SpcFeatureAlerts {
            features,
            has_alerts: true,
        });
        let expected_alert_description = "Drift has been detected for the following features:\n    test_feature_2: \n        Kind: Consecutive\n        Zone: Zone 1\n    test_feature_1: \n        Kind: Out of bounds\n        Zone: Zone 4\n".to_string();
        assert_eq!(&alert_description.len(), &expected_alert_description.len());

        unsafe {
            env::remove_var("OPSGENIE_API_URL");
            env::remove_var("OPSGENIE_API_KEY");
        }
    }

    #[test]
    fn test_construct_opsgenie_alert_description_empty() {
        unsafe {
            env::set_var("OPSGENIE_API_URL", "api_url");
            env::set_var("OPSGENIE_API_KEY", "api_key");
        }
        let features: HashMap<String, SpcFeatureAlert> = HashMap::new();
        let alerter = OpsGenieAlerter::new(
            "name",
            "repository",
            "1.0.0",
            &OpsGenieDispatchConfig {
                team: "test-team".to_string(),
            },
        )
        .unwrap();
        let alert_description = alerter.construct_alert_description(&SpcFeatureAlerts {
            features,
            has_alerts: true,
        });
        let expected_alert_description = "".to_string();
        assert_eq!(alert_description, expected_alert_description);
        unsafe {
            env::remove_var("OPSGENIE_API_URL");
            env::remove_var("OPSGENIE_API_KEY");
        }
    }

    #[tokio::test]
    async fn test_construct_opsgenie_alert_body() {
        // set env variables
        let download_server = mockito::Server::new_async().await;
        let url = download_server.url();

        // set env variables
        unsafe {
            env::set_var("OPSGENIE_API_URL", url);
            env::set_var("OPSGENIE_API_KEY", "api_key");
        }

        let ops_genie_team = "test-team";

        let expected_alert_body = json!(
                {
                    "message": "Model drift detected for test_repo/test_ml_model/1.0.0",
                    "description": "Features have drifted",
                    "responders":[
                        {"name":ops_genie_team, "type":"team"}
                    ],
                    "visibleTo":[
                        {"name":ops_genie_team, "type":"team"}
                    ],
                    "tags": ["Model Drift", "Scouter"],
                    "priority": "P1"
                }
        );
        let alerter = OpsGenieAlerter::new(
            "test_ml_model",
            "test_repo",
            "1.0.0",
            &OpsGenieDispatchConfig {
                team: ops_genie_team.to_string(),
            },
        )
        .unwrap();
        let alert_body = alerter.construct_alert_body("Features have drifted");
        assert_eq!(alert_body, expected_alert_body);
        unsafe {
            env::remove_var("OPSGENIE_API_URL");
            env::remove_var("OPSGENIE_API_KEY");
        }
    }

    #[tokio::test]
    async fn test_send_opsgenie_alerts() {
        let mut download_server = mockito::Server::new_async().await;
        let url = format!("{}/alerts", download_server.url());

        // set env variables
        unsafe {
            env::set_var("OPSGENIE_API_URL", url);
            env::set_var("OPSGENIE_API_KEY", "api_key");
        }

        let mock_get_path = download_server
            .mock("Post", "/alerts")
            .with_status(201)
            .create();

        let features = test_features_map();

        let dispatcher = AlertDispatcher::OpsGenie(HttpAlertDispatcher::new(
            OpsGenieAlerter::new(
                "name",
                "repository",
                "1.0.0",
                &OpsGenieDispatchConfig {
                    team: "test-team".to_string(),
                },
            )
            .unwrap(),
        ));
        let _ = dispatcher
            .process_alerts(&SpcFeatureAlerts {
                features,
                has_alerts: true,
            })
            .await;

        mock_get_path.assert();

        unsafe {
            env::remove_var("OPSGENIE_API_URL");
            env::remove_var("OPSGENIE_API_KEY");
        }
    }

    #[tokio::test]
    async fn test_send_console_alerts() {
        let features = test_features_map();
        let dispatcher =
            AlertDispatcher::Console(ConsoleAlertDispatcher::new("name", "repository", "1.0.0"));
        let result = dispatcher
            .process_alerts(&SpcFeatureAlerts {
                features,
                has_alerts: true,
            })
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_send_slack_alerts() {
        let mut download_server = mockito::Server::new_async().await;
        let url = download_server.url();

        // set env variables
        unsafe {
            env::set_var("SLACK_API_URL", url);
            env::set_var("SLACK_APP_TOKEN", "bot_token");
        }

        let mock_get_path = download_server
            .mock("Post", "/chat.postMessage")
            .with_status(201)
            .create();

        let features = test_features_map();

        let dispatcher = AlertDispatcher::Slack(HttpAlertDispatcher::new(
            SlackAlerter::new(
                "name",
                "repository",
                "1.0.0",
                &SlackDispatchConfig {
                    channel: "test-channel".to_string(),
                },
            )
            .unwrap(),
        ));
        let _ = dispatcher
            .process_alerts(&SpcFeatureAlerts {
                features,
                has_alerts: true,
            })
            .await;

        mock_get_path.assert();

        unsafe {
            env::remove_var("SLACK_API_URL");
            env::remove_var("SLACK_APP_TOKEN");
        }
    }

    #[tokio::test]
    async fn test_construct_slack_alert_body() {
        // set env variables
        let download_server = mockito::Server::new_async().await;
        let url = download_server.url();
        let slack_channel = "test_channel";

        unsafe {
            env::set_var("SLACK_API_URL", url);
            env::set_var("SLACK_APP_TOKEN", "bot_token");
        }
        let expected_alert_body = json!({
            "channel": slack_channel,
            "blocks": [
                {
                    "type": "header",
                    "text": {
                        "type": "plain_text",
                        "text": ":rotating_light: Drift Detected :rotating_light:",
                        "emoji": true
                    }
                },
                {
                    "type": "section",
                    "text": {
                      "type": "mrkdwn",
                      "text": "*Name*: name *Repository*: repository *Version*: 1.0.0",
                    }
                },
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": "*Features have drifted*"
                    },
                }
            ]
        });
        let alerter = SlackAlerter::new(
            "name",
            "repository",
            "1.0.0",
            &SlackDispatchConfig {
                channel: slack_channel.to_string(),
            },
        )
        .unwrap();
        let alert_body = alerter.construct_alert_body("*Features have drifted*");
        assert_eq!(alert_body, expected_alert_body);
        unsafe {
            env::remove_var("SLACK_API_URL");
            env::remove_var("SLACK_APP_TOKEN");
        }
    }

    #[test]
    fn test_console_dispatcher_returned_when_env_vars_not_set_opsgenie() {
        unsafe {
            env::remove_var("OPSGENIE_API_KEY");
        }
        let alert_config = SpcAlertConfig {
            dispatch_config: AlertDispatchConfig::OpsGenie(OpsGenieDispatchConfig {
                team: "test-team".to_string(),
            }),
            ..Default::default()
        };

        let config = SpcDriftConfig::new(
            Some("name".to_string()),
            Some("repository".to_string()),
            Some("1.0.0".to_string()),
            None,
            None,
            None,
            Some(alert_config),
            None,
        )
        .unwrap();
        let dispatcher = AlertDispatcher::new(&config).unwrap();

        assert!(
            matches!(dispatcher, AlertDispatcher::Console(_)),
            "Expected Console Dispatcher"
        );
    }

    #[test]
    fn test_console_dispatcher_returned_when_env_vars_not_set_slack() {
        unsafe {
            env::remove_var("SLACK_API_URL");
            env::remove_var("SLACK_APP_TOKEN");
        }

        let alert_config = SpcAlertConfig {
            dispatch_config: AlertDispatchConfig::Slack(SlackDispatchConfig {
                channel: "test-channel".to_string(),
            }),
            ..Default::default()
        };

        let config = SpcDriftConfig::new(
            Some("name".to_string()),
            Some("repository".to_string()),
            Some("1.0.0".to_string()),
            None,
            None,
            None,
            Some(alert_config),
            None,
        )
        .unwrap();

        let dispatcher = AlertDispatcher::new(&config).unwrap();
        assert!(
            matches!(dispatcher, AlertDispatcher::Console(_)),
            "Expected Console Dispatcher"
        );
    }

    #[test]
    fn test_slack_dispatcher_returned_when_env_vars_set() {
        unsafe {
            env::set_var("SLACK_API_URL", "url");
            env::set_var("SLACK_APP_TOKEN", "bot_token");
        }
        let alert_config = SpcAlertConfig {
            dispatch_config: AlertDispatchConfig::Slack(SlackDispatchConfig {
                channel: "test-channel".to_string(),
            }),
            ..Default::default()
        };

        let config = SpcDriftConfig::new(
            Some("name".to_string()),
            Some("repository".to_string()),
            Some("1.0.0".to_string()),
            None,
            None,
            None,
            Some(alert_config),
            None,
        )
        .unwrap();

        let dispatcher = AlertDispatcher::new(&config).unwrap();

        assert!(
            matches!(dispatcher, AlertDispatcher::Slack(_)),
            "Expected Slack Dispatcher"
        );

        unsafe {
            env::remove_var("SLACK_API_URL");
            env::remove_var("SLACK_APP_TOKEN");
        }
    }
}
