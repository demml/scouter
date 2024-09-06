use ndarray::{concatenate, s, Array1, Array2, Axis};
use ndarray_stats::CorrelationExt;

pub fn compute_correlation_matrix(drift_data: &Array2<f64>, target_idxs: &[usize]) -> Array2<f64> {
    let corr = drift_data.t().pearson_correlation().unwrap();

    println!("{:?}", corr);

    // return the correlation matrix at the target indices
    corr.select(Axis(0), target_idxs)
}

#[cfg(test)]
mod tests {
    use ndarray::{arr2, aview2};
    use ndarray_stats::CorrelationExt;

    use crate::core::stats::compute_correlation_matrix;

    #[test]
    fn test_multiple_linear_regression() {
        // Sample data
        let drift_data = arr2(&[
            [4., 1., 4.],
            [2., -1., 3.],
            [4., -1., 4.],
            [3., 1., 4.],
            [3., 1., 3.],
            [4., 1., 3.],
            [2., -1., 3.],
            [4., -1., 3.],
            [3., 1., 2.],
            [3., 1., 2.],
            [4., 1., 4.],
            [2., 1., 2.],
            [4., -1., 3.],
            [3., -1., 4.],
            [3., 1., 3.],
            [4., 1., 4.],
            [2., -1., 2.],
            [4., 1., 3.],
            [3., -1., 4.],
            [3., 1., 3.],
            [4., 1., 3.],
            [2., -1., 2.],
            [4., -1., 3.],
            [3., 1., 2.],
            [3., 1., 2.],
            [4., 1., 4.],
            [2., 1., 2.],
            [4., -1., 3.],
            [3., -1., 4.],
            [3., 1., 3.],
            [4., 1., 4.],
            [2., -1., 2.],
            [4., 1., 3.],
            [3., -1., 4.],
            [3., 1., 3.],
            [4., 1., 4.],
            [2., -1., 3.],
            [4., -1., 4.],
            [3., 1., 4.],
            [3., 1., 3.],
            [4., 1., 3.],
            [2., -1., 2.],
            [4., -1., 3.],
            [3., 1., 4.],
            [3., 1., 4.],
            [4., 1., 4.],
            [2., 1., 2.],
            [4., -1., 3.],
            [3., -1., 4.],
            [3., 1., 3.],
            [4., 1., 4.],
            [2., -1., 2.],
            [4., 1., 3.],
            [3., -1., 4.],
            [3., 1., 3.],
            [4., 1., 3.],
            [2., -1., 2.],
            [4., -1., 3.],
            [3., 1., 2.],
            [3., 1., 2.],
            [4., 1., 4.],
            [2., 1., 2.],
            [4., -1., 3.],
            [3., -1., 4.],
            [3., 1., 4.],
            [4., 1., 4.],
            [2., -1., 2.],
            [4., 1., 4.],
            [3., -1., 4.],
            [3., 1., 3.],
        ]);

        let target_idxs = [0, 2];
        let corrs = compute_correlation_matrix(&drift_data, &target_idxs);

        println!("{:?}", corrs);
    }
}
