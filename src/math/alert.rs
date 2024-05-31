use std::collections::HashMap;

use crate::types::_types::{Alert, AlertType, AlertZone};
use anyhow::Ok;
use anyhow::{Context, Result};
use ndarray::s;
use ndarray::ArrayView1;

pub fn check_zone_consecutive(
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

pub fn check_zone(
    value: f64,
    idx: usize,
    drift_array: &ArrayView1<f64>,
    consecutive_rule: usize,
    alternating_rule: usize,
    threshold: f64,
) -> Result<AlertType, anyhow::Error> {
    if (value == threshold || value == -threshold)
        && idx + 1 >= consecutive_rule as usize
        && consecutive_rule > 0
    {
        let start = idx + 1 - consecutive_rule as usize;
        let consecutive_alert = check_zone_consecutive(
            &drift_array.slice(s![start..=idx]),
            consecutive_rule,
            threshold,
        )?;

        if consecutive_alert {
            return Ok(AlertType::Consecutive);
        }
    } else if (value == threshold || value == -threshold)
        && idx + 1 >= alternating_rule as usize
        && alternating_rule > 0
    {
        let start = idx + 1 - alternating_rule as usize;
        let alternating_alert = check_zone_alternating(
            &drift_array.slice(s![start..=idx]),
            alternating_rule as usize,
            threshold,
        )?;

        if alternating_alert {
            return Ok(AlertType::Alternating);
        }
    }

    Ok(AlertType::AllGood)
}

pub fn convert_rules_to_vec(rule: String) -> Result<Vec<i32>, anyhow::Error> {
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

pub fn check_rule(
    drift_array: &ArrayView1<f64>,
    rule: String,
) -> Result<(Vec<Alert>, HashMap<usize, Vec<Vec<usize>>>), anyhow::Error> {
    let rule_vec = convert_rules_to_vec(rule)?;
    let mut alerts: Vec<Alert> = Vec::new();
    let mut alert_positions: HashMap<usize, Vec<Vec<usize>>> = HashMap::new();

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

            let alert = check_zone(
                *value,
                idx,
                drift_array,
                rule_vec[i] as usize,
                rule_vec[i + 1] as usize,
                threshold as f64,
            )
            .with_context(|| "Failed to check zone")?;

            if alert != AlertType::AllGood {
                // set the start position
                let start = if alert == AlertType::Consecutive {
                    idx + 1 - rule_vec[i] as usize
                } else {
                    idx + 1 - rule_vec[i + 1] as usize
                };

                // check if alert_positions already has a vector for the index
                if alert_positions.contains_key(&threshold) {
                    // check if the last alert position is the same as the current start position
                    let last_alert = alert_positions.get_mut(&threshold).unwrap().last().unwrap();
                    let last_start = last_alert[0];
                    let last_end = last_alert[1];

                    // check if last alert overlaps with current alert and merge if they do
                    if last_start <= idx && start <= last_end {
                        // merge index positions
                        let new_vec = vec![last_start, idx];
                        // update last alert
                        alert_positions.get_mut(&threshold).unwrap().pop();
                        alert_positions.get_mut(&threshold).unwrap().push(new_vec);
                    } else {
                        // push new alert position
                        alert_positions
                            .entry(threshold)
                            .or_insert_with(Vec::new)
                            .push(vec![start, idx]);
                    }
                } else {
                    // push new alert position
                    alert_positions
                        .entry(threshold)
                        .or_insert_with(Vec::new)
                        .push(vec![start, idx]);

                    // match zone
                    let zone = match threshold {
                        1 => AlertZone::Zone1.to_str(),
                        2 => AlertZone::Zone2.to_str(),
                        3 => AlertZone::Zone3.to_str(),
                        4 => AlertZone::OutOfBounds.to_str(),
                        _ => AlertZone::NotApplicable.to_str(),
                    };

                    alerts.push(Alert {
                        zone: zone.to_string(),
                        alert_type: alert.to_str(),
                    });
                }
            }
        }
    }

    Ok((alerts, alert_positions))
}

pub fn check_trend(
    drift_samples: &ArrayView1<f64>,
) -> Result<(Vec<Alert>, HashMap<usize, Vec<Vec<usize>>>), anyhow::Error> {
    let mut alerts: Vec<Alert> = Vec::new();
    let mut alert_positions: HashMap<usize, Vec<Vec<usize>>> = HashMap::new();

    drift_samples
        .windows(7)
        .into_iter()
        .enumerate()
        .for_each(|(count, window)| {
            // iterate over array and check if each value is increasing or decreasing
            let mut increasing = 0;
            let mut decreasing = 0;

            for i in 0..window.len() {
                if window[i] < window[i + 1] {
                    increasing += 1;
                } else if window[i] > window[i + 1] {
                    decreasing += 1;
                }
            }

            // check if increasing or decreasing values are greater than 7
            if increasing >= 7 || decreasing >= 7 {
                // check if increasing or decreasing values are greater than 7
                let start = count;
                let end = count + 6;

                // check if alert_positions already has a vector for the index
                if alert_positions.contains_key(&7) {
                    // check if the last alert position is the same as the current start position
                    let last_alert = alert_positions.get_mut(&7).unwrap().last().unwrap();
                    let last_start = last_alert[0];
                    let last_end = last_alert[1];

                    // check if last alert overlaps with current alert and merge if they do
                    if last_start <= end && start <= last_end {
                        // merge index positions
                        let new_vec = vec![last_start, end];
                        // update last alert
                        alert_positions.get_mut(&7).unwrap().pop();
                        alert_positions.get_mut(&7).unwrap().push(new_vec);
                    } else {
                        // push new alert position
                        alert_positions
                            .entry(7)
                            .or_insert_with(Vec::new)
                            .push(vec![start, end]);
                    }
                } else {
                    // push new alert position
                    alert_positions
                        .entry(7)
                        .or_insert_with(Vec::new)
                        .push(vec![start, end]);

                    // match zone
                    let zone = AlertZone::NotApplicable.to_str();

                    alerts.push(Alert {
                        zone: zone.to_string(),
                        alert_type: AlertType::Trend.to_str(),
                    });
                }
            }
        });

    Ok((alerts, alert_positions))
}

#[cfg(test)]
mod tests {

    use crate::types::_types::AlertRules;

    use super::*;
    use approx::relative_eq;
    use ndarray::Array;
    use ndarray_rand::rand_distr::Uniform;
    use ndarray_rand::RandomExt;

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
            3.0, 4.0, 0.0, -4.0,
        ];
        let drift_array = Array::from_vec(values.to_vec());
        let rule = AlertRules::Standard.to_str();

        let alert = check_rule(&drift_array.view(), rule).unwrap();

        assert_eq!(alert.0.len(), 3);
        assert_eq!(alert.1.get(&(1 as usize)), Some(&vec![vec![1, 10]]));
    }
}
