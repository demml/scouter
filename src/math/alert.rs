use std::collections::HashMap;

use crate::types::_types::{Alert, AlertType, AlertZone};

use anyhow::Ok;
use anyhow::{Context, Result};

use ndarray::s;
use ndarray::ArrayView1;

use std::collections::HashSet;

pub struct Alerter {
    pub alerts: HashSet<Alert>,
    pub alert_positions: HashMap<usize, Vec<Vec<usize>>>,
}

impl Alerter {
    pub fn new() -> Self {
        Alerter {
            alerts: HashSet::new(),
            alert_positions: HashMap::new(),
        }
    }

    pub fn check_zone_consecutive(
        self,
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
        self,
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
                && (drift_array[i] == threshold || drift_array[i] == -threshold)
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
        &self,
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
        mut self,
        key: usize,
        start: usize,
        end: usize,
    ) -> Result<(), anyhow::Error> {
        if self.alert_positions.contains_key(&key) {
            // check if the last alert position is the same as the current start position
            let last_alert = self.alert_positions.get_mut(&key).unwrap().last().unwrap();
            let last_start = last_alert[0];

            if self
                .has_overlap(last_alert, start, end)
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
        &self,
        value: f64,
        idx: usize,
        drift_array: &ArrayView1<f64>,
        consecutive_rule: usize,
        alternating_rule: usize,
        threshold: f64,
    ) -> Result<Vec<AlertType>, anyhow::Error> {
        let mut alert_types: Vec<AlertType> = Vec::new();
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

            if consecutive_alert {
                alert_types.push(AlertType::Consecutive);
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

            if alternating_alert {
                alert_types.push(AlertType::Alternating);
            }
        }

        alert_types.push(AlertType::AllGood);

        Ok(alert_types)
    }

    pub fn convert_rules_to_vec(&self, rule: String) -> Result<Vec<i32>, anyhow::Error> {
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
        mut self,
        drift_array: &ArrayView1<f64>,
        rule: String,
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

                let alerts = self
                    .check_zone(
                        *value,
                        idx,
                        drift_array,
                        rule_vec[i] as usize,
                        rule_vec[i + 1] as usize,
                        threshold as f64,
                    )
                    .with_context(|| "Failed to check zone")?;

                // get first item from alerts

                if alerts[0] == AlertType::AllGood {
                    continue;
                } else {
                    // update consecutive alerts
                    self.update_alert(idx + 1 - rule_vec[i] as usize, idx, threshold, alerts[0]);

                    // update alternating alerts
                    self.update_alert(
                        idx + 1 - rule_vec[i + 1] as usize,
                        idx,
                        threshold,
                        alerts[1],
                    );
                }
            }
        }

        Ok(())
    }

    pub fn update_alert(
        mut self,
        start: usize,
        idx: usize,
        threshold: usize,
        alert: AlertType,
    ) -> Result<(), anyhow::Error> {
        self.insert_alert(threshold, start, idx)
            .with_context(|| "Failed to insert alert")?;

        let zone = match threshold {
            1 => AlertZone::Zone1.to_str(),
            2 => AlertZone::Zone2.to_str(),
            3 => AlertZone::Zone3.to_str(),
            4 => AlertZone::OutOfBounds.to_str(),
            _ => AlertZone::NotApplicable.to_str(),
        };

        self.alerts.insert(Alert {
            zone: zone.to_string(),
            alert_type: alert.to_str(),
        });

        Ok(())
    }

    pub fn check_trend(self, drift_array: &ArrayView1<f64>) -> Result<(), anyhow::Error> {
        let mut alerts: Vec<Alert> = Vec::new();
        let mut alert_positions: HashMap<usize, Vec<Vec<usize>>> = HashMap::new();

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
                    // no reason to push multiple alerts

                    if alerts.len() == 0 {
                        alerts.push(Alert {
                            zone: AlertZone::NotApplicable.to_str(),
                            alert_type: AlertType::Trend.to_str(),
                        });
                    }
                    let start = count;
                    let end = count + 6;

                    if increasing >= 6 {
                        self.insert_alert(0, start, end).unwrap();
                    } else if decreasing >= 6 {
                        self.insert_alert(1, start, end).unwrap();
                    }
                }
            });

        Ok((alerts, alert_positions))
    }
}

#[cfg(test)]
mod tests {

    use crate::types::_types::AlertRules;

    use super::*;
    use ndarray::Array;

    #[test]
    fn test_alerting_consecutive() {
        // write tests for all alerts
        let values = [0.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = check_zone_consecutive(&drift_array.view(), 5, threshold).unwrap();
        assert_eq!(result, true);

        let values = [0.0, 1.0, 1.0, -1.0, 1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = check_zone_consecutive(&drift_array.view(), 5, threshold).unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_alerting_alternating() {
        let values = [0.0, 1.0, -1.0, 1.0, -1.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = check_zone_alternating(&drift_array.view(), 5, threshold).unwrap();
        assert_eq!(result, true);

        let values = [0.0, 1.0, -1.0, 1.0, 0.0, 1.0];
        let drift_array = Array::from_vec(values.to_vec());
        let threshold = 1.0;

        let result = check_zone_alternating(&drift_array.view(), 5, threshold).unwrap();
        assert_eq!(result, false);
    }

    #[test]
    fn test_convert_rule() {
        let vec_of_ints = convert_rules_to_vec(AlertRules::Standard.to_str()).unwrap();
        assert_eq!(vec_of_ints, [8, 16, 4, 8, 2, 4, 1, 1,]);
    }

    #[test]
    fn test_check_rule() {
        let values = [
            0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0, 1.0, -2.0, 2.0, 0.0, 0.0, 3.0, 3.0,
            3.0, 4.0, 0.0, -4.0, 3.0, -3.0, 3.0, -3.0, 3.0, -3.0,
        ];
        let drift_array = Array::from_vec(values.to_vec());
        let rule = AlertRules::Standard.to_str();

        let alert = check_rule(&drift_array.view(), rule).unwrap();

        println!("{:?}", alert);

        assert_eq!(alert.0.len(), 3);
        assert_eq!(alert.1.get(&(1 as usize)), Some(&vec![vec![1, 10]]));
    }

    #[test]
    fn test_check_trend() {
        let values = [
            0.0, 0.0, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.2, 0.3, 0.4,
            0.5, 0.6, 0.7,
        ];
        let drift_samples = Array::from_vec(values.to_vec());

        let alert = check_trend(&drift_samples.view()).unwrap();
        assert_eq!(alert.0.len(), 1);
        assert_eq!(
            alert.1.get(&(0 as usize)),
            Some(&vec![vec![1, 7], vec![13, 19]])
        );
    }
}
