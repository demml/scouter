use ndarray::prelude::*;
use ndarray_stats::CorrelationExt;
use std::collections::BTreeMap;

// compute_feature_correlations computes the correlation between features in a 2D array
//
// # Arguments
//
// * `data` - A 2D array of data
// * `features` - A vector of feature names
//
// # Returns
//
// A BTreeMap of feature names to a BTreeMap of other feature names to the correlation value
pub fn compute_feature_correlations(
    data: &ArrayView2<f64>,
    features: &[String],
) -> BTreeMap<String, BTreeMap<String, f64>> {
    let mut feature_correlations: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();
    let correlations = data.t().pearson_correlation().unwrap();

    features.iter().enumerate().for_each(|(i, feature)| {
        let mut feature_correlation: BTreeMap<String, f64> = BTreeMap::new();
        features.iter().enumerate().for_each(|(j, other_feature)| {
            if i != j {
                let value = correlations[[i, j]];
                // extract the correlation value
                feature_correlation.insert(other_feature.clone(), value);
            }
        });
        feature_correlations.insert(feature.clone(), feature_correlation);
    });

    feature_correlations
}

#[cfg(test)]
mod tests {

    use super::*;
    use ndarray::stack;
    use ndarray_rand::rand::thread_rng;
    use ndarray_rand::rand_distr::{Distribution, Normal};

    fn generate_correlated_arrays(size: usize, correlation: f64) -> (Array1<f64>, Array1<f64>) {
        let mut rng = thread_rng();
        let normal = Normal::new(0.0, 1.0).unwrap();

        let x: Array1<f64> = Array1::from_iter((0..size).map(|_| normal.sample(&mut rng)));

        let y: Array1<f64> = Array1::from_iter((0..size).map(|i| {
            correlation * x[i] + (1.0 - correlation.powi(2)).sqrt() * normal.sample(&mut rng)
        }));

        (x, y)
    }

    #[test]
    fn test_correlation_2d() {
        // generate first set
        let (x1, y1) = generate_correlated_arrays(20000, 0.75);

        // generate second set
        let (x2, y2) = generate_correlated_arrays(20000, 0.33);

        let (x3, y3) = generate_correlated_arrays(20000, -0.80);

        // combine into 4 columns
        let data = stack![Axis(1), x1, y1, x2, y2, x3, y3];
        let features = vec![
            "x1".to_string(),
            "y1".to_string(),
            "x2".to_string(),
            "y2".to_string(),
            "x3".to_string(),
            "y3".to_string(),
        ];

        let correlations = compute_feature_correlations(&data.view(), &features);

        assert!((correlations["x1"]["y1"] - 0.75).abs() < 0.1);
        assert!((correlations["x2"]["y2"] - 0.33).abs() < 0.1);
        assert!((correlations["x3"]["y3"] + 0.80).abs() < 0.1);
    }
}
