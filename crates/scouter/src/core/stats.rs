use ndarray::prelude::*;
use rayon::prelude::*;
use std::{collections::BTreeMap, sync::Arc};

fn compute_correlation(x: &ArrayView1<f64>, y: &ArrayView1<f64>) -> f64 {
    if x.len() != y.len() {
        panic!("Arrays must have the same length");
    }

    let x_mean = x.mean().unwrap();
    let y_mean = y.mean().unwrap();

    let numerator = x
        .iter()
        .zip(y.iter())
        .map(|(&xi, &yi)| (xi - x_mean) * (yi - y_mean))
        .sum::<f64>();

    let x_variance = x.iter().map(|&xi| (xi - x_mean).powi(2)).sum::<f64>();

    let y_variance = y.iter().map(|&yi| (yi - y_mean).powi(2)).sum::<f64>();

    numerator / (x_variance.sqrt() * y_variance.sqrt())
}

pub fn compute_feature_correlations(
    data: &ArrayView2<f64>,
    features: &[String],
) -> BTreeMap<String, BTreeMap<String, f64>> {
    let data_arc = Arc::new(data.clone()); // Clone once, then use Arc for cheap cloning
    let mut feature_correlations: BTreeMap<String, BTreeMap<String, f64>> = BTreeMap::new();

    let correlations: Vec<Vec<f64>> = data
        .axis_iter(Axis(1))
        .into_par_iter()
        .enumerate()
        .map(|(i, column)| {
            let data = Arc::clone(&data_arc);
            data.axis_iter(Axis(1))
                .into_par_iter()
                .enumerate()
                .map(|(j, other_column)| {
                    if i == j {
                        1.0
                    } else {
                        compute_correlation(&column, &other_column)
                    }
                })
                .collect::<Vec<f64>>()
        })
        .collect();

    for (i, feature) in features.iter().enumerate() {
        let mut feature_correlation: BTreeMap<String, f64> = BTreeMap::new();
        for (j, other_feature) in features.iter().enumerate() {
            if i != j {
                feature_correlation.insert(other_feature.clone(), correlations[i][j]);
            }
        }
        feature_correlations.insert(feature.clone(), feature_correlation);
    }
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
    fn test_correlation_1d() {
        let (x, y) = generate_correlated_arrays(1000, 0.75);
        let computed_correlation = compute_correlation(&x.view(), &y.view());
        let expected_correlation = 0.75;
        assert!((computed_correlation - expected_correlation).abs() < 0.1);

        let (x, y) = generate_correlated_arrays(1000, 0.33);
        let computed_correlation = compute_correlation(&x.view(), &y.view());
        let expected_correlation = 0.33;
        assert!((computed_correlation - expected_correlation).abs() < 0.1);
    }

    #[test]
    fn test_negative_correlation_1d() {
        let x = array![1., 2., 3., 4., 5.];
        let y = array![5., 4., 3., 2., 1.];
        assert!((compute_correlation(&x.view(), &y.view()) + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_correlation_2d() {
        // generate first set
        let (x1, y1) = generate_correlated_arrays(10000, 0.75);

        // generate second set
        let (x2, y2) = generate_correlated_arrays(10000, 0.33);

        // combine into 4 columns
        let data = stack![Axis(1), x1, y1, x2, y2];
        let features = vec![
            "x1".to_string(),
            "y1".to_string(),
            "x2".to_string(),
            "y2".to_string(),
        ];

        let correlations = compute_feature_correlations(&data.view(), &features);

        assert!((correlations["x1"]["y1"] - 0.75).abs() < 0.1);
        assert!((correlations["x2"]["y2"] - 0.33).abs() < 0.1);
    }
}
