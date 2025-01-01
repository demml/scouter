use scouter_error::MonitorError;
use scouter_types::FeatureMap;
use ndarray::{Array, Array2};
use pyo3::prelude::*;
use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use std::collections::BTreeSet;
use std::collections::HashMap;

pub trait CategoricalFeatureHelpers {
    // creates a feature map from a 2D array
    //
    // # Arguments
    //
    // * `features` - A vector of feature names
    // * `array` - A 2D array of string values
    //
    // # Returns
    //
    // A feature map
    fn create_feature_map(
        &self,
        features: &[String],
        array: &[Vec<String>],
    ) -> Result<FeatureMap, MonitorError> {
        // check if features and array are the same length
        if features.len() != array.len() {
            return Err(MonitorError::ShapeMismatchError(
                "Features and array are not the same length".to_string(),
            ));
        };

        let feature_map = array
            .par_iter()
            .enumerate()
            .map(|(i, col)| {
                let unique = col
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                let mut map = HashMap::new();
                for (j, item) in unique.iter().enumerate() {
                    map.insert(item.to_string(), j);

                    // check if j is last index
                    if j == unique.len() - 1 {
                        // insert missing value
                        map.insert("missing".to_string(), j + 1);
                    }
                }

                (features[i].to_string(), map)
            })
            .collect::<HashMap<_, _>>();

        Ok(FeatureMap {
            features: feature_map,
        })
    }

    fn convert_strings_to_ndarray_f32(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f32>, MonitorError>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(MonitorError::MissingFeatureError(
                "Features provided do not exist in feature map".to_string(),
            ));
        }

        let data = features
            .par_iter()
            .enumerate()
            .map(|(i, feature)| {
                let map = feature_map.features.get(feature).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = array[i]
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f32)
                    .collect::<Vec<_>>();

                col
            })
            .collect::<Vec<_>>();

        let data = Array::from_shape_vec((features.len(), array[0].len()), data.concat())
            .map_err(|e| MonitorError::ArrayError(e.to_string()))?;

        Ok(data.t().to_owned())
    }

    fn convert_strings_to_ndarray_f64(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f64>, MonitorError>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(MonitorError::MissingFeatureError(
                "Features provided do not exist in feature map".to_string(),
            ));
        }
        let data = features
            .par_iter()
            .enumerate()
            .map(|(i, feature)| {
                let map = feature_map.features.get(feature).unwrap();

                // attempt to set feature. If not found, set to missing
                let col = array[i]
                    .iter()
                    .map(|x| *map.get(x).unwrap_or(map.get("missing").unwrap()) as f64)
                    .collect::<Vec<_>>();
                col
            })
            .collect::<Vec<_>>();

        let data = Array::from_shape_vec((features.len(), array[0].len()), data.concat())
            .map_err(|e| MonitorError::ArrayError(e.to_string()))?;

        Ok(data.t().to_owned())
    }
}
