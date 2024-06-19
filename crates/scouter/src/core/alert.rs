use std::collections::HashMap;

use crate::types::_types::{Alert, AlertType, AlertZone};
use anyhow::Ok;
use anyhow::{Context, Result};
use ndarray::s;
use ndarray::ArrayView1;

use std::collections::HashSet;

// Struct for holding stateful Alert information
#[derive(Clone)]
pub struct Alerter {
    pub alerts: HashSet<Alert>,
    pub alert_positions: HashMap<usize, Vec<Vec<usize>>>,
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
    pub fn new() -> Self {
        Alerter {
            alerts: HashSet::new(),
            alert_positions: HashMap::new(),
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
    ) -> Result<bool, anyhow::Error> {
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
    ) -> Result<bool, anyhow::Error> {
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

    pub fn has_overlap(
        last_entry: &Vec<usize>,
        start: usize,
        end: usize,
    ) -> Result<bool, anyhow::Error> {
        let last_start = last_entry[0];
        let last_end = last_entry[1];

        let has_overlap = last_start <= end && start <= last_end;

        Ok(has_overlap)
    }

    pub fn insert_alert(
        &mut self,
        key: usize,
        start: usize,
        end: usize,
    ) -> Result<(), anyhow::Error> {
        if self.alert_positions.contains_key(&key) {
            // check if the last alert position is the same as the current start position
            let last_alert = self.alert_positions.get_mut(&key).unwrap().last().unwrap();
            let last_start = last_alert[0];

            if Alerter::has_overlap(last_alert, start, end)
                .with_context(|| "Failed to check overlap")?
            {
                let new_vec = vec![last_start, end];
                self.alert_positions.get_mut(&key).unwrap().pop();
                self.alert_positions.get_mut(&key).unwrap().push(new_vec);
            } else {
                let vec = vec![start, end];
                self.alert_positions.get_mut(&key).unwrap().push(vec);
            }
        } else {
            // push new alert position
            self.alert_positions
                .entry(key)
                .or_insert_with(Vec::new)
                .push(vec![start, end]);
        }

        Ok(())
    }

    pub fn check_zone(
        &mut self,
        value: f64,
        idx: usize,
        drift_array: &ArrayView1<f64>,
        consecutive_rule: usize,
        alternating_rule: usize,
        threshold: f64,
    ) -> Result<(), anyhow::Error> {
        // test consecutive first
        if (value == threshold || value == -threshold)
            && idx + 1 >= consecutive_rule as usize
            && consecutive_rule > 0
        {
            let start = idx + 1 - consecutive_rule as usize;
            let consecutive_alert = self.check_zone_consecutive(
                &drift_array.slice(s![start..=idx]),
                consecutive_rule,
                threshold,
            )?;

            // update alerts
            if consecutive_alert {
                self.update_alert(
                    idx + 1 - consecutive_rule,
                    idx,
                    threshold as usize,
                    AlertType::Consecutive,
                )
                .with_context(|| "Failed to update consecutive alert indices")?;
            }
        }

        // check alternating
        if (value == threshold || value == -threshold)
            && idx + 1 >= alternating_rule as usize
            && alternating_rule > 0
        {
            let start = idx + 1 - alternating_rule as usize;
            let alternating_alert = self.check_zone_alternating(
                &drift_array.slice(s![start..=idx]),
                alternating_rule as usize,
                threshold,
            )?;

            // update alerts
            if alternating_alert {
                self.update_alert(
                    idx + 1 - alternating_rule,
                    idx,
                    threshold as usize,
                    AlertType::Alternating,
                )
                .with_context(|| "Failed to update consecutive alert indices")?;
            }
        }

        Ok(())
    }

    pub fn convert_rules_to_vec(&self, rule: &str) -> Result<Vec<i32>, anyhow::Error> {
        let rule_chars = rule.split(" ");

        let rule_vec = rule_chars
            .collect::<Vec<&str>>()
            .into_iter()
            .map(|ele| ele.parse::<i32>().with_context(|| "Failed to parse rule"))
            .collect::<Result<Vec<i32>, anyhow::Error>>()?;

        // assert rule_vec.len() == 7
        let rule_vec_len = rule_vec.len();
        if rule_vec_len != 8 {
            return Err(anyhow::anyhow!(
                "Rule must be 8 characters long. Found: {}",
                rule_vec_len
            ));
        }

        Ok(rule_vec)
    }

    pub fn check_rule_for_alert(
        &mut self,
        drift_array: &ArrayView1<f64>,
        rule: &str,
    ) -> Result<(), anyhow::Error> {
        let rule_vec = self.convert_rules_to_vec(rule)?;

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
                )
                .with_context(|| "Failed to check zone")?;
            }
        }

        Ok(())
    }

    pub fn update_alert(
        &mut self,
        start: usize,
        idx: usize,
        threshold: usize,
        alert: AlertType,
    ) -> Result<(), anyhow::Error> {
        self.insert_alert(threshold, start, idx)
            .with_context(|| "Failed to insert alert")?;

        if threshold == 4 {
            self.alerts.insert(Alert {
                zone: AlertZone::OutOfBounds.to_str(),
                kind: AlertType::OutOfBounds.to_str(),
            });
        } else {
            let zone = match threshold {
                1 => AlertZone::Zone1.to_str(),
                2 => AlertZone::Zone2.to_str(),
                3 => AlertZone::Zone3.to_str(),
                _ => AlertZone::NotApplicable.to_str(),
            };

            self.alerts.insert(Alert {
                zone: zone.to_string(),
                kind: alert.to_str(),
            });
        }

        Ok(())
    }

    pub fn check_trend(&mut self, drift_array: &ArrayView1<f64>) -> Result<(), anyhow::Error> {
        drift_array
            .windows(7)
            .into_iter()
            .enumerate()
            .for_each(|(count, window)| {
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
                    self.alerts.insert(Alert {
                        zone: AlertZone::NotApplicable.to_str(),
                        kind: AlertType::Trend.to_str(),
                    });

                    let start = count;
                    let end = count + 6;

                    if increasing >= 6 {
                        self.insert_alert(5, start, end).unwrap();
                    } else if decreasing >= 6 {
                        self.insert_alert(6, start, end).unwrap();
                    }
                }
            });

        Ok(())
    }
}

//pub fn generate_alert(
//    drift_array: &ArrayView1<f64>,
//    rule: &str,
//) -> Result<(HashSet<Alert>, HashMap<usize, Vec<Vec<usize>>>), anyhow::Error> {
//    let mut alerter = Alerter::new();
//
//    // check for alerts
//    alerter
//        .check_rule_for_alert(&drift_array.view(), rule)
//        .with_context(|| "Failed to check rule for alert")?;
//
//    // check for trend
//    alerter
//        .check_trend(&drift_array.view())
//        .with_context(|| "Failed to check trend")?;
//
//    Ok((alerter.alerts, alerter.alert_positions))
//}
//
//pub fn generate_alerts(
//    drift_array: &ArrayView2<f64>,
//    features: Vec<String>,
//    alert_rule: String,
//) -> Result<HashMap<String, (HashSet<Alert>, HashMap<usize, Vec<Vec<usize>>>)>, anyhow::Error> {
//    let mut alert_map = HashMap::new();
//
//    // check for alerts
//    let alerts = drift_array
//        .axis_iter(Axis(1))
//        .into_par_iter()
//        .map(|col| {
//            // check for alerts and errors
//            Ok(generate_alert(&col, &alert_rule)
//                .with_context(|| "Failed to check rule for alert")?)
//        })
//        .collect::<Vec<Result<(HashSet<Alert>, HashMap<usize, Vec<Vec<usize>>>), anyhow::Error>>>();
//
//    //zip the alerts with the features
//    for (feature, alert) in features.iter().zip(alerts.iter()) {
//        // unwrap the alert, should should have already been checked
//        let result = alert.as_ref().unwrap();
//        alert_map.insert(feature.to_string(), result.clone());
//    }
//
//    Ok(alert_map)
//}

#[cfg(test)]
mod tests {

    use crate::types::_types::AlertRules;

    use super::*;
    use ndarray::Array;

    #[test]
    fn test_alerting_consecutive() {
        let alerter = Alerter::new();
        // write tests for all alerts
        let values = [0.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_consecutive(&drift_array.view(), 5, threshold)
            .unwrap();
        assert_eq!(result, true);

        let values = [0.0, 1.0, 1.0, -1.0, 1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_consecutive(&drift_array.view(), 5, threshold)
            .unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_alerting_alternating() {
        let alerter = Alerter::new();
        let values = [0.0, 1.0, -1.0, 1.0, -1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_alternating(&drift_array.view(), 5, threshold)
            .unwrap();
        assert_eq!(result, true);

        let values = [0.0, 1.0, -1.0, 1.0, 0.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = alerter
            .check_zone_alternating(&drift_array.view(), 5, threshold)
            .unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_convert_rule() {
        let alerter = Alerter::new();
        let vec_of_ints = alerter
            .convert_rules_to_vec(&AlertRules::Standard.to_str())
            .unwrap();
        assert_eq!(vec_of_ints, [8, 16, 4, 8, 2, 4, 1, 1,]);
    }

    #[test]
    fn test_check_rule() {
        let mut alerter = Alerter::new();
        let values = [
            0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, -2.0, 2.0, 0.0, 0.0, 3.0, 3.0,
            3.0, 4.0, 0.0, -4.0, 3.0, -3.0, 3.0, -3.0, 3.0, -3.0,
        ];
        let drift_array = Array::from_vec(values.to_vec());
        let rule = AlertRules::Standard.to_str();

        alerter
            .check_rule_for_alert(&drift_array.view(), &rule)
            .unwrap();

        let alert = alerter.alert_positions;

        assert_eq!(alert.get(&1).unwrap(), &vec![vec![1, 10]]);
        assert_eq!(alert.get(&3).unwrap(), &vec![vec![15, 17], vec![20, 26]]);
        assert_eq!(alert.get(&4).unwrap(), &vec![vec![18, 18], vec![20, 20]]);

        assert_eq!(alerter.alerts.len(), 4);
    }

    #[test]
    fn test_check_trend() {
        let mut alerter = Alerter::new();
        let values = [
            0.0, 0.0, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.2, 0.3, 0.4,
            0.5, 0.6, 0.7,
        ];
        let drift_samples = Array::from_vec(values.to_vec());

        alerter.check_trend(&drift_samples.view()).unwrap();

        // get first alert
        let alert = alerter.alerts.iter().next().unwrap();

        assert_eq!(alert.zone, "NA");
        assert_eq!(alert.kind, "Trend");
    }

    #[test]
    fn test_generate_alerts() {
        // has alerts
        // create 20, 3 vector

        let vec = [
            [0.0, 0.0, 4.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 2.0],
            [0.0, 0.0, -2.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 1.0],
        ];

        // combine all 3 into array
        let drift_samples = Array::from_shape_vec((20, 3), vec.to_vec());

        println!("{:?}", drift_samples);
    }
}
