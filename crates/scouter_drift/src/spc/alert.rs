use ndarray::s;
use ndarray::{ArrayView1, ArrayView2, Axis};
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use scouter_error::AlertError;
use scouter_types::spc::{AlertZone, SpcAlert, SpcAlertRule, SpcAlertType, SpcFeatureAlerts};
use std::collections::HashSet;

// Struct for holding stateful Alert information
#[derive(Clone)]
pub struct Alerter {
    pub alerts: HashSet<SpcAlert>,
    pub alert_rule: SpcAlertRule,
}

impl Alerter {
    // Create a new instance of the Alerter struct
    //
    // Sets both alerts (hashset) and alert positions (hashmap)
    // Alerts is a collection of unique alert types
    // Alert positions is a hashmap of keys (alert zones) and their corresponding alert start and stop indices
    // Keys:
    //  1 - Zone 1 alerts
    //  2 - Zone 2 alerts
    //  3 - Zone 3 alerts
    //  4 - Zone 4 alerts (out of bounds)
    //  5 - Increasing trend alerts
    //  6 - Decreasing trend alerts
    pub fn new(alert_rule: SpcAlertRule) -> Self {
        Alerter {
            alerts: HashSet::new(),
            alert_rule,
        }
    }

    // Check if the drift array has a consecutive zone alert for negative or positive values
    //
    // drift_array - ArrayView1<f64> - The drift array to check
    // zone_consecutive_rule - usize - The number of consecutive values to check for
    // threshold - f64 - The threshold value to check against
    pub fn check_zone_consecutive(
        &self,
        drift_array: &ArrayView1<f64>,
        zone_consecutive_rule: usize,
        threshold: f64,
    ) -> Result<bool, AlertError> {
        let pos_count = drift_array.iter().filter(|&x| *x >= threshold).count();

        let neg_count = drift_array.iter().filter(|&x| *x <= -threshold).count();

        if pos_count >= zone_consecutive_rule || neg_count >= zone_consecutive_rule {
            return Ok(true);
        }

        Ok(false)
    }

    pub fn check_zone_alternating(
        &self,
        drift_array: &ArrayView1<f64>,
        zone_alt_rule: usize,
        threshold: f64,
    ) -> Result<bool, AlertError> {
        // check for consecutive alternating values

        let mut last_val = 0.0;
        let mut alt_count = 0;

        for i in 0..drift_array.len() {
            if drift_array[i] == 0.0 {
                last_val = 0.0;
                alt_count = 0;
                continue;
            } else if drift_array[i] != last_val
                && (drift_array[i] >= threshold || drift_array[i] <= -threshold)
            {
                alt_count += 1;
                if alt_count >= zone_alt_rule {
                    return Ok(true);
                }
            } else {
                last_val = 0.0;
                alt_count = 0;
                continue;
            }

            last_val = drift_array[i];
        }

        Ok(false)
    }

    pub fn has_overlap(last_entry: &[usize], start: usize, end: usize) -> Result<bool, AlertError> {
        let last_start = last_entry[0];
        let last_end = last_entry[1];

        let has_overlap = last_start <= end && start <= last_end;

        Ok(has_overlap)
    }

    pub fn check_zone(
        &mut self,
        value: f64,
        idx: usize,
        drift_array: &ArrayView1<f64>,
        consecutive_rule: usize,
        alternating_rule: usize,
        threshold: f64,
    ) -> Result<(), AlertError> {
        // test consecutive first
        if (value == threshold || value == -threshold)
            && idx + 1 >= consecutive_rule
            && consecutive_rule > 0
        {
            let start = idx + 1 - consecutive_rule;
            let consecutive_alert = self.check_zone_consecutive(
                &drift_array.slice(s![start..=idx]),
                consecutive_rule,
                threshold,
            )?;

            // update alerts
            if consecutive_alert {
                self.update_alert(threshold as usize, SpcAlertType::Consecutive)?;
            }
        }

        // check alternating
        if (value == threshold || value == -threshold)
            && idx + 1 >= alternating_rule
            && alternating_rule > 0
        {
            let start = idx + 1 - alternating_rule;
            let alternating_alert = self.check_zone_alternating(
                &drift_array.slice(s![start..=idx]),
                alternating_rule,
                threshold,
            )?;

            // update alerts
            if alternating_alert {
                self.update_alert(threshold as usize, SpcAlertType::Alternating)?;
            }
        }

        Ok(())
    }

    pub fn convert_rules_to_vec(&self, rule: &str) -> Result<Vec<i32>, AlertError> {
        let rule_chars = rule.split(' ');

        let rule_vec = rule_chars
            .collect::<Vec<&str>>()
            .into_iter()
            .map(|ele| {
                ele.parse::<i32>()
                    .map_err(|e| AlertError::CreateError(e.to_string()))
            })
            .collect::<Result<Vec<i32>, AlertError>>()?;

        // assert rule_vec.len() == 7
        let rule_vec_len = rule_vec.len();
        if rule_vec_len != 8 {
            return Err(AlertError::CreateError(format!(
                "Rule vector length is not 8, found {}",
                rule_vec_len
            )));
        }

        Ok(rule_vec)
    }

    pub fn check_process_rule_for_alert(
        &mut self,
        drift_array: &ArrayView1<f64>,
    ) -> Result<(), AlertError> {
        let rule_vec = self.convert_rules_to_vec(&self.alert_rule.rule)?;

        // iterate over each value in drift array
        for (idx, value) in drift_array.iter().enumerate() {
            // iterate over rule vec and step by 2 (consecutive and alternating rules for each zone)
            for i in (0..=6).step_by(2) {
                let threshold = match i {
                    0 => 1,
                    2 => 2,
                    4 => 3,
                    6 => 4,
                    _ => 0,
                };

                self.check_zone(
                    *value,
                    idx,
                    drift_array,
                    rule_vec[i] as usize,
                    rule_vec[i + 1] as usize,
                    threshold as f64,
                )?;
            }
        }

        Ok(())
    }

    pub fn update_alert(
        &mut self,
        threshold: usize,
        alert: SpcAlertType,
    ) -> Result<(), AlertError> {
        let alert_zone = match threshold {
            1 => AlertZone::Zone1,
            2 => AlertZone::Zone2,
            3 => AlertZone::Zone3,
            4 => AlertZone::Zone4,
            _ => AlertZone::NotApplicable,
        };

        // skip if the zone is not in the process rule
        if !self.alert_rule.zones_to_monitor.contains(&alert_zone) {
            return Ok(());
        }

        if alert_zone == AlertZone::Zone4 {
            self.alerts.insert(SpcAlert {
                zone: alert_zone,
                kind: SpcAlertType::OutOfBounds,
            });
        } else {
            self.alerts.insert(SpcAlert {
                zone: alert_zone,
                kind: alert,
            });
        }

        Ok(())
    }

    pub fn check_trend(&mut self, drift_array: &ArrayView1<f64>) -> Result<(), AlertError> {
        drift_array.windows(7).into_iter().for_each(|window| {
            // iterate over array and check if each value is increasing or decreasing
            let mut increasing = 0;
            let mut decreasing = 0;

            // iterate through
            for i in 1..window.len() {
                if window[i] > window[i - 1] {
                    increasing += 1;
                } else if window[i] < window[i - 1] {
                    decreasing += 1;
                }
            }

            if increasing >= 6 || decreasing >= 6 {
                self.alerts.insert(SpcAlert {
                    zone: AlertZone::NotApplicable,
                    kind: SpcAlertType::Trend,
                });
            }
        });

        Ok(())
    }
}

impl Default for Alerter {
    fn default() -> Self {
        let rule = SpcAlertRule::default();
        Alerter {
            alerts: HashSet::new(),
            alert_rule: rule,
        }
    }
}

pub fn generate_alert(
    drift_array: &ArrayView1<f64>,
    rule: &SpcAlertRule,
) -> Result<HashSet<SpcAlert>, AlertError> {
    let mut alerter = Alerter::new(rule.clone());

    alerter
        .check_process_rule_for_alert(&drift_array.view())
        .map_err(|e| {
            AlertError::CreateError(format!("Failed to check process rule for alert: {}", e))
        })?;

    alerter.check_trend(&drift_array.view())?;

    Ok(alerter.alerts)
}

/// Generate alerts for each feature in the drift array
///
/// # Arguments
/// drift_array - ArrayView2<f64> - The drift array to check for alerts (column order should match feature order)
/// features - Vec<String> - The features to check for alerts (feature order should match drift array column order)
/// alert_rule - AlertRule - The alert rule to check against
///
/// Returns a Result<FeatureAlerts, AlertError>
///
pub fn generate_alerts(
    drift_array: &ArrayView2<f64>,
    features: &[String],
    rule: &SpcAlertRule,
) -> Result<SpcFeatureAlerts, AlertError> {
    let mut has_alerts: bool = false;

    // check for alerts
    let alerts = drift_array
        .axis_iter(Axis(1))
        .into_par_iter()
        .map(|col| {
            // check for alerts and errors
            generate_alert(&col, rule)
        })
        .collect::<Vec<Result<HashSet<SpcAlert>, AlertError>>>();

    // Calculate correlation matrix when there are alerts
    if alerts
        .iter()
        .any(|alert| !alert.as_ref().unwrap().is_empty())
    {
        // get correlation matrix
        has_alerts = true;
    };

    let mut feature_alerts = SpcFeatureAlerts::new(has_alerts);

    //zip the alerts with the features
    for (feature, alert) in features.iter().zip(alerts.iter()) {
        // unwrap the alert, should should have already been checked
        let alerts = alert.as_ref().unwrap();
        //let alerts: Vec<SpcAlert> = alerts.iter().cloned().collect();

        feature_alerts.insert_feature_alert(feature, alerts.to_owned());
    }

    Ok(feature_alerts)
}

#[cfg(test)]
mod tests {

    use scouter_types::spc::SpcAlertRule;

    use super::*;
    use ndarray::arr2;
    use ndarray::Array;

    #[test]
    fn test_alerting_consecutive() {
        let alerter = Alerter::default();
        // write tests for all alerts
        let values = [0.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_consecutive(&drift_array.view(), 5, threshold)
            .unwrap();
        assert!(result);

        let values = [0.0, 1.0, 1.0, -1.0, 1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_consecutive(&drift_array.view(), 5, threshold)
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_alerting_alternating() {
        let alerter = Alerter::default();
        let values = [0.0, 1.0, -1.0, 1.0, -1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_alternating(&drift_array.view(), 5, threshold)
            .unwrap();
        assert!(result);

        let values = [0.0, 1.0, -1.0, 1.0, 0.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_alternating(&drift_array.view(), 5, threshold)
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_convert_rule() {
        let alerter = Alerter::default();
        let vec_of_ints = alerter
            .convert_rules_to_vec(&SpcAlertRule::default().rule)
            .unwrap();
        assert_eq!(vec_of_ints, [8, 16, 4, 8, 2, 4, 1, 1,]);
    }

    #[test]
    fn test_check_rule() {
        let mut alerter = Alerter::default();
        let values = [
            0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, -2.0, 2.0, 0.0, 0.0, 3.0, 3.0,
            3.0, 4.0, 0.0, -4.0, 3.0, -3.0, 3.0, -3.0, 3.0, -3.0,
        ];
        let drift_array = Array::from_vec(values.to_vec());
        alerter
            .check_process_rule_for_alert(&drift_array.view())
            .unwrap();

        assert_eq!(alerter.alerts.len(), 4);
    }

    #[test]
    fn test_check_rule_zones_to_monitor() {
        let zones_to_monitor = [AlertZone::Zone1, AlertZone::Zone4].to_vec();
        let process = SpcAlertRule::new(None, Some(zones_to_monitor));
        let mut alerter = Alerter::new(process);

        let values = [
            0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, -2.0, 2.0, 0.0, 0.0, 3.0, 3.0,
            3.0, 4.0, 0.0, -4.0, 3.0, -3.0, 3.0, -3.0, 3.0, -3.0,
        ];
        let drift_array = Array::from_vec(values.to_vec());

        alerter
            .check_process_rule_for_alert(&drift_array.view())
            .unwrap();

        assert_eq!(alerter.alerts.len(), 2);
    }

    #[test]
    fn test_check_trend() {
        let mut alerter = Alerter::default();
        let values = [
            0.0, 0.0, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.2, 0.3, 0.4,
            0.5, 0.6, 0.7,
        ];
        let drift_samples = Array::from_vec(values.to_vec());

        alerter.check_trend(&drift_samples.view()).unwrap();

        // get first alert
        let alert = alerter.alerts.iter().next().unwrap();

        assert_eq!(alert.zone, AlertZone::NotApplicable);
        assert_eq!(alert.kind, SpcAlertType::Trend);
    }

    #[test]
    fn test_generate_process_alerts() {
        // has alerts
        // create 20, 3 vector

        let drift_array = arr2(&[
            [0.0, 0.0, 4.0, 4.0],
            [0.0, 1.0, 1.0, 1.0],
            [1.0, 0.0, -1.0, -1.0],
            [0.0, 1.1, 2.0, 2.0],
            [2.0, 0.0, -2.0, -2.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 2.1, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [2.0, 1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.0, 2.1, 1.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [1.0, 0.0, 1.0, 1.0],
        ]);

        // assert shape is 16,3
        assert_eq!(drift_array.shape(), &[14, 4]);

        let features = vec![
            "feature1".to_string(),
            "feature2".to_string(),
            "feature3".to_string(),
            "feature4".to_string(),
        ];

        let rule = SpcAlertRule::default();

        let alerts = generate_alerts(&drift_array.view(), &features, &rule).unwrap();

        let feature1 = alerts.features.get("feature1").unwrap();
        let feature2 = alerts.features.get("feature2").unwrap();
        let feature3 = alerts.features.get("feature3").unwrap();
        let feature4 = alerts.features.get("feature4").unwrap();

        // assert feature 1 is has an empty hash set
        assert_eq!(feature1.alerts.len(), 0);
        assert_eq!(feature1.alerts.len(), 0);

        // assert feature 3 has 2 alerts
        assert_eq!(feature3.alerts.len(), 2);

        assert_eq!(feature4.alerts.len(), 2);

        // assert feature 2 has 0 alert
        assert_eq!(feature2.alerts.len(), 0);
    }
}
