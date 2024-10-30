use crate::core::error::ProfilerError;
use crate::core::error::ScouterError;
use crate::core::profile::types::DataProfile;
use crate::core::profile::types::{CharStats, Distinct, FeatureProfile, StringStats, WordStats};
use crate::core::stats::compute_feature_correlations;
use crate::core::utils::create_feature_map;
use ndarray::Array2;
use rayon::prelude::*;
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
    ) -> Result<DataProfile, ProfilerError> {
        let profiles = self
            .create_string_profile(&string_array, &string_features)
            .map_err(|e| {
                ProfilerError::StringProfileError(format!("Failed to create string profile: {}", e))
            })?;

        let correlations: Option<HashMap<String, HashMap<String, f32>>> = if compute_correlations {
            let converted_array = self
                .convert_string_vec_to_num_array(&string_array, &string_features)
                .map_err(|e| {
                    ProfilerError::ConversionError(format!(
                        "Failed to convert string array to numeric array: {}",
                        e
                    ))
                })?;

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
    ) -> Result<Array2<f32>, ProfilerError> {
        let feature_map = create_feature_map(string_features, string_array).map_err(|e| {
            ProfilerError::FeatureMapError(format!(
                "Failed to create feature map for string array: {}",
                e
            ))
        })?;

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
        )
        .map_err(|e| ProfilerError::ArrayError(e.to_string()))?;

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
    ) -> Result<Vec<FeatureProfile>, ScouterError> {
        let string_profiler = StringProfiler::new();
        let string_profile = string_profiler
            .compute_2d_stats(string_array, string_features)
            .map_err(|_e| ScouterError::StringProfileError(_e.to_string()))?;

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
                    correlations: None,
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
