use crate::types::_types::{Alert, AlertType, AlertZone};
use anyhow::{Context, Result};
use ndarray::s;
use ndarray::ArrayView1;

pub fn check_zone_consecutive(
    drift_array: &ArrayView1<f64>,
    zone_consecutive_rule: usize,
    threshold: f64,
) -> Result<bool, anyhow::Error> {
    let pos_count = drift_array.iter().filter(|&x| *x == threshold).count();

    let neg_count = drift_array.iter().filter(|&x| *x == -threshold).count();

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
        let consecutive_alert =
            check_zone_consecutive(&drift_array.slice(s![start..=idx]), consecutive_rule, 2.0)?;

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
            2.0,
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
    if rule_vec_len != 7 {
        return Err(anyhow::anyhow!(
            "Rule must be 9 characters long. Found: {}",
            rule_vec_len
        ));
    }

    Ok(rule_vec)
}

pub fn check_rule(drift_array: &ArrayView1<f64>, rule: String) -> Result<Alert, anyhow::Error> {
    let rule_vec = convert_rules_to_vec(rule)?;

    let zone1_consecutive_rule = rule_vec[0];
    let zone1_alternating_rule = rule_vec[1];
    let zone2_consecutive_rule = rule_vec[2];
    let zone2_alternating_rule = rule_vec[3];
    let zone3_consecutive_rule = rule_vec[4];
    let zone3_alternating_rule = rule_vec[5];
    let out_of_bounds = rule_vec[6];

    let mut out_of_bounds_count = 0;

    // check zone 2

    for (idx, value) in drift_array.iter().enumerate() {
        // check for out of bounds (zone 3)
        if *value == 4.0 || *value == -4.0 {
            out_of_bounds_count += 1;
            if out_of_bounds_count >= out_of_bounds {
                return Ok(Alert::new(
                    AlertType::OutOfBounds.as_str(),
                    AlertZone::OutOfBounds.as_str(),
                ));
            }

            // check zone 2
        }

        let zone3_alert = check_zone(
            *value,
            idx,
            &drift_array,
            zone3_consecutive_rule as usize,
            zone3_alternating_rule as usize,
            2.0,
        )?;

        if zone3_alert != AlertType::AllGood {
            return Ok(Alert::new(zone3_alert.as_str(), AlertZone::Zone3.as_str()));
        }

        // check zone 1
        let zone2_alert = check_zone(
            *value,
            idx,
            &drift_array,
            zone2_consecutive_rule as usize,
            zone2_alternating_rule as usize,
            1.0,
        )?;

        if zone2_alert != AlertType::AllGood {
            return Ok(Alert::new(zone2_alert.as_str(), AlertZone::Zone2.as_str()));
        }

        // check zone 0
        let zone1_alert = check_zone(
            *value,
            idx,
            &drift_array,
            zone1_consecutive_rule as usize,
            zone1_alternating_rule as usize,
            0.0,
        )?;

        if zone1_alert != AlertType::AllGood {
            return Ok(Alert::new(zone1_alert.as_str(), AlertZone::Zone1.as_str()));
        }
    }

    // convert

    return Ok(Alert::new(
        AlertType::AllGood.as_str(),
        AlertZone::NotApplicable.as_str(),
    ));
}
