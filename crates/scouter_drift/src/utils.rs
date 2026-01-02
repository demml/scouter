use crate::error::DriftError;
use ndarray::{Array, Array2};
use rayon::iter::IndexedParallelIterator;
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::ParallelIterator;
use scouter_types::FeatureMap;
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
    ) -> Result<FeatureMap, DriftError> {
        // check if features and array are the same length
        if features.len() != array.len() {
            return Err(DriftError::FeatureLengthError);
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
                    map.insert(item.to_string(), j as i32);

                    // check if j is last index
                    if j == unique.len() - 1 {
                        // insert missing value
                        map.insert("missing".to_string(), j as i32 + 1);
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
    ) -> Result<Array2<f32>, DriftError>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(DriftError::FeatureNotExistError);
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
            .map_err(DriftError::ShapeError)?;

        Ok(data.t().to_owned())
    }

    fn convert_strings_to_ndarray_f64(
        &self,
        features: &Vec<String>,
        array: &[Vec<String>],
        feature_map: &FeatureMap,
    ) -> Result<Array2<f64>, DriftError>
where {
        // check if features in feature_map.features.keys(). If any feature is not found, return error
        let features_not_exist = features
            .iter()
            .map(|x| feature_map.features.contains_key(x))
            .position(|x| !x);

        if features_not_exist.is_some() {
            return Err(DriftError::FeatureNotExistError);
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
            .map_err(DriftError::ShapeError)?;

        Ok(data.t().to_owned())
    }
}

#[cfg(test)]
mod tests {

    use crate::utils::CategoricalFeatureHelpers;

    pub struct TestStruct;
    impl CategoricalFeatureHelpers for TestStruct {}

    #[test]
    fn test_create_feature_map_base() {
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "hello".to_string(),
                "blah".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
                "hello".to_string(),
                "blah".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let feature_map = TestStruct
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);
        assert_eq!(feature_map.features.get("feature_2").unwrap().len(), 6);
    }

    #[test]
    fn test_create_array_from_string() {
        let string_vec = vec![
            vec![
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
                "e".to_string(),
            ],
            vec![
                "a".to_string(),
                "a".to_string(),
                "a".to_string(),
                "b".to_string(),
                "b".to_string(),
            ],
        ];

        let string_features = vec!["feature_1".to_string(), "feature_2".to_string()];

        let feature_map = TestStruct
            .create_feature_map(&string_features, &string_vec)
            .unwrap();

        assert_eq!(feature_map.features.len(), 2);

        let f32_array = TestStruct
            .convert_strings_to_ndarray_f32(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(f32_array.shape(), &[5, 2]);

        let f64_array = TestStruct
            .convert_strings_to_ndarray_f64(&string_features, &string_vec, &feature_map)
            .unwrap();

        assert_eq!(f64_array.shape(), &[5, 2]);
    }
}
