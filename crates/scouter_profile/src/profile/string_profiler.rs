use crate::error::DataProfileError;
use crate::profile::types::DataProfile;
use crate::profile::types::{CharStats, Distinct, FeatureProfile, StringStats, WordStats};
use crate::stats::compute_feature_correlations;
use chrono::Utc;
use ndarray::Array2;
use rayon::prelude::*;
use scouter_types::create_feature_map;
use std::collections::BTreeMap;
use std::collections::HashMap;

pub struct StringProfiler {}

impl StringProfiler {
    pub fn new() -> Self {
        StringProfiler {}
    }

    pub fn process_string_array<F>(
        &self,
        string_array: Vec<Vec<String>>,
        string_features: Vec<String>,
        compute_correlations: bool,
    ) -> Result<DataProfile, DataProfileError> {
        let profiles = self.create_string_profile(&string_array, &string_features)?;

        let correlations: Option<HashMap<String, HashMap<String, f32>>> = if compute_correlations {
            let converted_array =
                self.convert_string_vec_to_num_array(&string_array, &string_features)?;

            let correlations =
                compute_feature_correlations(&converted_array.view(), &string_features);
            Some(correlations)
        } else {
            None
        };

        let features: BTreeMap<String, FeatureProfile> = profiles
            .iter()
            .map(|profile| {
                let mut profile = profile.clone();

                if let Some(correlations) = correlations.as_ref() {
                    let correlation = correlations.get(&profile.id);
                    if let Some(correlation) = correlation {
                        profile.add_correlations(correlation.clone());
                    }
                }

                (profile.id.clone(), profile)
            })
            .collect();

        Ok(DataProfile { features })
    }

    pub fn convert_string_vec_to_num_array(
        &self,
        string_array: &[Vec<String>],
        string_features: &[String],
    ) -> Result<Array2<f32>, DataProfileError> {
        let feature_map = create_feature_map(string_features, string_array)?;

        // zip and map string_array and string_features
        let arrays = string_array
            .par_iter()
            .enumerate()
            .map(|(i, col)| {
                let map = feature_map.features.get(&string_features[i]).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = col
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f32)
                    .collect::<Vec<_>>();
                Array2::from_shape_vec((col.len(), 1), col).unwrap()
            })
            .collect::<Vec<_>>();

        let num_array = ndarray::concatenate(
            ndarray::Axis(1),
            &arrays.iter().map(|a| a.view()).collect::<Vec<_>>(),
        )?;

        Ok(num_array)
    }

    // Create a string profile for a 2D array of strings
    //
    // # Arguments
    //
    // * `string_array` - A 2D array of strings
    // * `string_features` - A vector of feature names
    // # Returns
    //
    // * `Vec<FeatureProfile>` - A vector of feature profiles
    pub fn create_string_profile(
        &self,
        string_array: &[Vec<String>],
        string_features: &[String],
    ) -> Result<Vec<FeatureProfile>, DataProfileError> {
        let string_profiler = StringProfiler::new();
        let string_profile = string_profiler.compute_2d_stats(string_array, string_features)?;

        Ok(string_profile)
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
    pub fn compute_stats(&self, array: &Vec<String>) -> Result<StringStats, DataProfileError> {
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
    ) -> Result<Vec<FeatureProfile>, DataProfileError> {
        // zip the string features with the array

        let map_vec = array
            .par_iter()
            .enumerate()
            .map(|(i, col)| {
                let feature = &string_features[i];
                let stats = self.compute_stats(col)?;

                Ok(FeatureProfile {
                    id: feature.to_string(),
                    string_stats: Some(stats),
                    numeric_stats: None,
                    timestamp: Utc::now(),
                    correlations: None,
                })
            })
            .collect::<Result<Vec<FeatureProfile>, DataProfileError>>()?;

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
