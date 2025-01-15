use chrono::Utc;
use cron::Schedule;
use pyo3::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[pyclass(eq)]
#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum CommonCrons {
    Every1Minute,
    Every5Minutes,
    Every15Minutes,
    Every30Minutes,
    EveryHour,
    Every6Hours,
    Every12Hours,
    EveryDay,
    EveryWeek,
}

#[pymethods]
impl CommonCrons {
    #[getter]
    pub fn cron(&self) -> String {
        match self {
            CommonCrons::Every1Minute => "0 * * * * * *".to_string(),
            CommonCrons::Every5Minutes => {
                "0 0,5,10,15,20,25,30,35,40,45,50,55 * * * * *".to_string()
            }
            CommonCrons::Every15Minutes => "0 0,15,30,45 * * * * *".to_string(),
            CommonCrons::Every30Minutes => "0 0,30 * * * * *".to_string(),
            CommonCrons::EveryHour => "0 0 * * * *".to_string(),
            CommonCrons::Every6Hours => "0 0 */6 * * *".to_string(),
            CommonCrons::Every12Hours => "0 0 */12 * * *".to_string(),
            CommonCrons::EveryDay => "0 0 0 * * *".to_string(),
            CommonCrons::EveryWeek => "0 0 0 * * SUN".to_string(),
        }
    }

    pub fn get_next(&self) -> String {
        match self {
            CommonCrons::Every1Minute => {
                let schedule = Schedule::from_str("0 * * * * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::Every5Minutes => {
                let schedule =
                    Schedule::from_str("0 0,5,10,15,20,25,30,35,40,45,50,55 * * * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::Every15Minutes => {
                let schedule = Schedule::from_str("0 0,15,30,45 * * * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::Every30Minutes => {
                let schedule = Schedule::from_str("0 0,30 * * * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::EveryHour => {
                let schedule = Schedule::from_str("0 0 * * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::Every6Hours => {
                let schedule = Schedule::from_str("0 0 */6 * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::Every12Hours => {
                let schedule = Schedule::from_str("0 0 */12 * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::EveryDay => {
                let schedule = Schedule::from_str("0 0 0 * * *").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
            CommonCrons::EveryWeek => {
                let schedule = Schedule::from_str("0 0 0 * * SUN").unwrap();
                schedule.upcoming(Utc).next().unwrap().to_string()
            }
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
        let cron = CommonCrons::Every30Minutes;

        // check cron
        assert_eq!(cron.cron(), "0 0,30 * * * * *");

        // check next
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_hour_cron() {
        let cron = CommonCrons::EveryHour;
        // check cron
        assert_eq!(cron.cron(), "0 0 * * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_6_hours_cron() {
        let cron = CommonCrons::Every6Hours;
        // check cron
        assert_eq!(cron.cron(), "0 0 */6 * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_12_hours_cron() {
        let cron = CommonCrons::Every12Hours;
        // check cron
        assert_eq!(cron.cron(), "0 0 */12 * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_day_cron() {
        let cron = CommonCrons::EveryDay;
        // check cron
        assert_eq!(cron.cron(), "0 0 0 * * *");
        let _next = cron.get_next();
    }

    #[test]
    fn test_every_week_cron() {
        let cron = CommonCrons::EveryWeek;
        // check cron
        assert_eq!(cron.cron(), "0 0 0 * * SUN");
        let _next = cron.get_next();
    }

    #[test]
    fn test_cron_schedule_cron() {
        let cron = CommonCrons::Every1Minute;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::Every5Minutes;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::Every15Minutes;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::Every30Minutes;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::EveryHour;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::Every6Hours;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::Every12Hours;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::EveryDay;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());

        let cron = CommonCrons::EveryWeek;
        let schedule = Schedule::from_str(&cron.cron()).unwrap();
        let next = schedule.upcoming(Utc).next().unwrap();
        assert_eq!(next.to_string(), cron.get_next());
    }
}
