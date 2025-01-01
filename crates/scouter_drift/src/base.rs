use scouter_types::dispatch::AlertDispatchType;




pub struct DriftArgs {
    pub name: String,
    pub repository: String,
    pub version: String,
    pub dispatch_type: AlertDispatchType,
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::CategoricalFeatureHelpers;
    use std::str::FromStr;
    use scouter_types::ValidateAlertConfig;

    pub struct TestStruct;
    impl ValidateAlertConfig for TestStruct {}
    impl CategoricalFeatureHelpers for TestStruct {}

    #[test]
    fn test_resolve_schedule_base() {
        let valid_schedule = "0 0 5 * * *".to_string(); // Every day at 5:00 AM

        let result = TestStruct::resolve_schedule(Some(valid_schedule));

        assert_eq!(result, "0 0 5 * * *".to_string());

        let invalid_schedule = "invalid_cron".to_string();

        let default_schedule = EveryDay::new().cron;

        let result = TestStruct::resolve_schedule(Some(invalid_schedule));

        assert_eq!(result, default_schedule);
    }

    #[test]
    fn test_drift_type_from_str_base() {
        assert_eq!(DriftType::from_str("SPC").unwrap(), DriftType::Spc);
        assert_eq!(DriftType::from_str("PSI").unwrap(), DriftType::Psi);
        assert_eq!(DriftType::from_str("CUSTOM").unwrap(), DriftType::Custom);
        assert!(DriftType::from_str("INVALID").is_err());
    }

    #[test]
    fn test_drift_type_value_base() {
        assert_eq!(DriftType::Spc.to_string(), "Spc");
        assert_eq!(DriftType::Psi.to_string(), "Psi");
        assert_eq!(DriftType::Custom.to_string(), "Custom");
    }

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
