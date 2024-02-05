use numpy::ndarray::ArrayView1;
use rayon::prelude::*;
use std::sync::{Arc, Mutex};

/// Compute the bins for a 1d array of data
///
/// # Arguments
///
/// * `data` - A 1d array of data
/// * `num_bins` - The number of bins to use
///
/// # Returns
/// * `Vec<f64>` - A vector of bins
pub fn compute_bins(data: &ArrayView1<f64>, num_bins: u32) -> Vec<f64> {
    // find the min and max of the data

    let min = data.into_iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max = data.into_iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

    // create a vector of bins
    let mut bins = Vec::<f64>::with_capacity(num_bins as usize);

    // compute the bin width
    let bin_width = (max - min) / num_bins as f64;

    // create the bins
    for i in 0..num_bins {
        bins.push(min + bin_width * i as f64);
    }

    // return the bins
    bins
}

/// Compute the bin counts for a 1d array of data
///
/// # Arguments
///
/// * `data` - A 1d array of data
/// * `bins` - A vector of bins
///
/// # Returns
/// * `Vec<i32>` - A vector of bin counts
pub fn compute_bin_counts(data: &ArrayView1<f64>, bins: &[f64]) -> Vec<i32> {
    // create a vector to hold the bin counts
    let bin_counts = Arc::new(Mutex::new(vec![0; bins.len()]));
    let max_bin = bins.last().unwrap();

    data.into_par_iter().for_each(|datum| {
        // iterate over the bins
        for (i, bin) in bins.iter().enumerate() {
            if bin != max_bin {
                let bin_range = bin..&bins[i + 1];

                if bin_range.contains(&datum) {
                    bin_counts.lock().unwrap()[i] += 1;
                    break;
                }
                continue;
            } else if bin == max_bin {
                if datum > bin {
                    bin_counts.lock().unwrap()[i] += 1;
                    break;
                }
                continue;
            } else {
                continue;
            }
        }
    });

    return bin_counts.lock().unwrap().to_vec();
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray_rand::rand_distr::Normal;
    use ndarray_rand::RandomExt;
    use numpy::ndarray::Array1;

    #[test]
    fn test_compute_bins() {
        let test_array = Array1::random(10_000, Normal::new(0.0, 1.0).unwrap());

        let bins = compute_bins(&test_array.view(), 10);

        let counts = compute_bin_counts(&test_array.view(), &bins);
        assert_eq!(10000, counts.iter().sum::<i32>());
    }
}
