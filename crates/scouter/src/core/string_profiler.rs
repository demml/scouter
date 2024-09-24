use crate::core::error::ProfilerError;
use crate::utils::types::{CharStats, Distinct, FeatureProfile, StringStats, WordStats};
use rayon::prelude::*;
use std::collections::HashMap;

pub struct StringProfiler {}

impl StringProfiler {
    pub fn new() -> Self {
        StringProfiler {}
    }

    // Compute the statistics for a string array
    //
    // # Arguments
    //
    // * `array` - A vector of strings
    //
    // # Returns
    //
    // * `StringStats` - A struct containing the statistics for the string array
    pub fn compute_stats(&self, array: &Vec<String>) -> Result<StringStats, ProfilerError> {
        let mut unique = HashMap::new();

        let count = array.len();
        let mut lengths = Vec::new();

        for item in array {
            *unique.entry(item).or_insert(0) += 1;
            lengths.push(item.chars().count());
        }

        // unique count
        let unique_count = unique.len();

        // median
        lengths.sort();
        let median = lengths[lengths.len() / 2];

        let char_stats = CharStats {
            min_length: lengths[0],
            max_length: lengths[lengths.len() - 1],
            mean_length: lengths.iter().sum::<usize>() as f64 / count as f64,
            median_length: median,
        };

        // need to get distinct for each word
        let mut word_stats = HashMap::new();
        for (key, value) in unique.iter() {
            word_stats.insert(
                key.to_string(),
                Distinct {
                    count: *value,
                    percent: *value as f64 / count as f64,
                },
            );
        }

        let string_stats = StringStats {
            distinct: Distinct {
                count: unique_count,
                percent: (unique_count as f64 / count as f64) * 100.0,
            },
            char_stats,
            word_stats: WordStats { words: word_stats },
        };

        Ok(string_stats)
    }

    pub fn compute_2d_stats(
        &self,
        array: &[Vec<String>],
        string_features: &[String],
    ) -> Result<Vec<FeatureProfile>, ProfilerError> {
        // zip the string features with the array

        let map_vec = array
            .par_iter()
            .enumerate()
            .map(|(i, col)| {
                let feature = &string_features[i];
                let stats = self
                    .compute_stats(col)
                    .map_err(|_| ProfilerError::StringStatsError)
                    .unwrap();

                FeatureProfile {
                    id: feature.to_string(),
                    string_stats: Some(stats),
                    numeric_stats: None,
                    timestamp: chrono::Utc::now().naive_utc(),
                }
            })
            .collect::<Vec<FeatureProfile>>();

        Ok(map_vec)
    }
}

impl Default for StringProfiler {
    fn default() -> Self {
        StringProfiler::new()
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_compute_stats() {
        let string_profiler = StringProfiler::new();
        let array = vec![
            vec![
                "hello".to_string(),
                "world".to_string(),
                "world".to_string(),
            ],
            vec!["blah".to_string(), "foo".to_string(), "world".to_string()],
        ];

        let stats = string_profiler
            .compute_2d_stats(&array, &["feature1".to_string(), "feature2".to_string()])
            .unwrap();

        assert_eq!(stats.len(), 2);
    }
}
