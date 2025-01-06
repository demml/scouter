use chrono::Utc;
use cron::Schedule;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Every1Minute {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Every1Minute {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 * * * * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Every5Minutes {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Every5Minutes {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0,5,10,15,20,25,30,35,40,45,50,55 * * * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Every15Minutes {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Every15Minutes {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0,15,30,45 * * * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Every30Minutes {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Every30Minutes {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0,30 * * * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct EveryHour {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl EveryHour {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0 * * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Every6Hours {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Every6Hours {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0 */6 * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Every12Hours {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl Every12Hours {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0 */12 * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct EveryDay {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl EveryDay {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0 0 * * *".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct EveryWeek {
    #[pyo3(get, set)]
    pub cron: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl EveryWeek {
    #[new]
    pub fn new() -> Self {
        Self {
            cron: "0 0 0 * * SUN".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        let schedule = Schedule::from_str(&self.cron).unwrap();
        schedule.upcoming(Utc).next().unwrap().to_string()
    }
}

#[pyclass]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[allow(non_snake_case)]
pub struct CommonCron {
    #[pyo3(get)]
    pub EVERY_1_MINUTE: String,

    #[pyo3(get)]
    pub EVERY_5_MINUTES: String,

    #[pyo3(get)]
    pub EVERY_15_MINUTES: String,

    #[pyo3(get)]
    pub EVERY_30_MINUTES: String,

    #[pyo3(get)]
    pub EVERY_HOUR: String,

    #[pyo3(get)]
    pub EVERY_6_HOURS: String,

    #[pyo3(get)]
    pub EVERY_12_HOURS: String,

    #[pyo3(get)]
    pub EVERY_DAY: String,

    #[pyo3(get)]
    pub EVERY_WEEK: String,
}

#[pymethods]
#[allow(clippy::new_without_default)]
impl CommonCron {
    #[new]
    pub fn new() -> Self {
        Self {
            EVERY_1_MINUTE: Every1Minute::new().cron,
            EVERY_5_MINUTES: Every5Minutes::new().cron,
            EVERY_15_MINUTES: Every15Minutes::new().cron,
            EVERY_30_MINUTES: Every30Minutes::new().cron,
            EVERY_HOUR: EveryHour::new().cron,
            EVERY_6_HOURS: Every6Hours::new().cron,
            EVERY_12_HOURS: Every12Hours::new().cron,
            EVERY_DAY: EveryDay::new().cron,
            EVERY_WEEK: EveryWeek::new().cron,
        }
    }
}

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum TimeInterval {
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    OneHour,
    ThreeHours,
    SixHours,
    TwelveHours,
    TwentyFourHours,
    TwoDays,
    FiveDays,
}

impl TimeInterval {
    pub fn to_minutes(&self) -> i32 {
        match self {
            TimeInterval::FiveMinutes => 5,
            TimeInterval::FifteenMinutes => 15,
            TimeInterval::ThirtyMinutes => 30,
            TimeInterval::OneHour => 60,
            TimeInterval::ThreeHours => 180,
            TimeInterval::SixHours => 360,
            TimeInterval::TwelveHours => 720,
            TimeInterval::TwentyFourHours => 1440,
            TimeInterval::TwoDays => 2880,
            TimeInterval::FiveDays => 7200,
        }
    }

    pub fn from_string(time_window: &str) -> TimeInterval {
        match time_window {
            "5minute" => TimeInterval::FiveMinutes,
            "15minute" => TimeInterval::FifteenMinutes,
            "30minute" => TimeInterval::ThirtyMinutes,
            "1hour" => TimeInterval::OneHour,
            "3hour" => TimeInterval::ThreeHours,
            "6hour" => TimeInterval::SixHours,
            "12hour" => TimeInterval::TwelveHours,
            "24hour" => TimeInterval::TwentyFourHours,
            "2day" => TimeInterval::TwoDays,
            "5day" => TimeInterval::FiveDays,
            _ => TimeInterval::SixHours,
        }
    }
}

impl fmt::Display for TimeInterval {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TimeInterval::FiveMinutes => write!(f, "5minute"),
            TimeInterval::FifteenMinutes => write!(f, "15minute"),
            TimeInterval::ThirtyMinutes => write!(f, "30minute"),
            TimeInterval::OneHour => write!(f, "1hour"),
            TimeInterval::ThreeHours => write!(f, "3hour"),
            TimeInterval::SixHours => write!(f, "6hour"),
            TimeInterval::TwelveHours => write!(f, "12hour"),
            TimeInterval::TwentyFourHours => write!(f, "24hour"),
            TimeInterval::TwoDays => write!(f, "2day"),
            TimeInterval::FiveDays => write!(f, "5day"),
        }
    }
}

// test crons

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_30_minutes_cron() {
        let cron = Every30Minutes::new();
        // check cron
        assert_eq!(cron.cron, "0 0,30 * * * * *");

        // check next
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_hour_cron() {
        let cron = EveryHour::new();
        // check cron
        assert_eq!(cron.cron, "0 0 * * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_6_hours_cron() {
        let cron = Every6Hours::new();
        // check cron
        assert_eq!(cron.cron, "0 0 */6 * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_12_hours_cron() {
        let cron = Every12Hours::new();
        // check cron
        assert_eq!(cron.cron, "0 0 */12 * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_day_cron() {
        let cron = EveryDay::new();
        // check cron
        assert_eq!(cron.cron, "0 0 0 * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_week_cron() {
        let cron = EveryWeek::new();
        // check cron
        assert_eq!(cron.cron, "0 0 0 * * SUN");
        let _next = cron.get_next();
    }

    #[test]
    fn test_common_cron_cron() {
        let cron = CommonCron::new();
        // check cron
        assert_eq!(cron.EVERY_30_MINUTES, "0 0,30 * * * * *");
        assert_eq!(cron.EVERY_HOUR, "0 0 * * * *");
        assert_eq!(cron.EVERY_6_HOURS, "0 0 */6 * * *");
        assert_eq!(cron.EVERY_12_HOURS, "0 0 */12 * * *");
        assert_eq!(cron.EVERY_DAY, "0 0 0 * * *");
        assert_eq!(cron.EVERY_WEEK, "0 0 0 * * SUN");
    }

    #[test]
    fn test_cron_schedule_cron() {
        let cron = Every1Minute::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = Every5Minutes::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = Every15Minutes::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = Every30Minutes::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = EveryHour::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = Every6Hours::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = Every12Hours::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = EveryDay::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = EveryWeek::new();
        let schedule = Schedule::from_str(&cron.cron).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());
    }
}
