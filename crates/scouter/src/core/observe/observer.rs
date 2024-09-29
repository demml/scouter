use ndarray::Array1;
use ndarray_stats::interpolate::Nearest;
use ndarray_stats::Quantile1dExt;
use noisy_float::types::n64;
use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
#[derive(Clone, Debug)]
struct LatencyMetrics {
    #[pyo3(get)]
    p5: f64,

    #[pyo3(get)]
    p25: f64,

    #[pyo3(get)]
    p50: f64,

    #[pyo3(get)]
    p95: f64,

    #[pyo3(get)]
    p99: f64,
}

#[pyclass]
#[derive(Clone, Debug)]
struct RouteLatency {
    #[pyo3(get)]
    request_latency: Vec<f64>,

    #[pyo3(get)]
    request_count: i64,

    #[pyo3(get)]
    error_count: i64,

    #[pyo3(get)]
    error_latency: f64,

    #[pyo3(get)]
    status_codes: HashMap<usize, i64>,
}

#[pyclass]
#[derive(Clone)]
struct Observer {
    #[pyo3(get)]
    request_count: i64,

    #[pyo3(get)]
    error_count: i64,

    #[pyo3(get)]
    request_latency: HashMap<String, RouteLatency>,
}

#[pymethods]
impl Observer {
    #[new]
    fn new() -> Self {
        Observer {
            request_count: 0,
            error_count: 0,
            request_latency: HashMap::new(),
        }
    }

    fn increment_request_count(&mut self) {
        self.request_count += 1;
    }

    fn increment_error_count(&mut self, status: &str) {
        if status != "OK" {
            self.error_count += 1;
        }
    }

    fn update_route_latency(
        &mut self,
        route: &str,
        latency: f64,
        status: &str,
        status_code: usize,
    ) {
        // handling OK status
        if status == "OK" {
            // insert latency for route if it doesn't exist, otherwise increment
            if let Some(route_latency) = self.request_latency.get_mut(route) {
                route_latency.request_latency.push(latency);
                route_latency.request_count += 1;
            } else {
                self.request_latency.insert(
                    route.to_string(),
                    RouteLatency {
                        request_latency: vec![latency],
                        request_count: 1,
                        error_count: 0,
                        error_latency: 0.0,
                        status_codes: HashMap::new(),
                    },
                );
            }

        // handling errors
        } else {
            // insert latency for route if it doesn't exist, otherwise increment
            if let Some(route_latency) = self.request_latency.get_mut(route) {
                route_latency.error_latency += latency;
                route_latency.error_count += 1;
            } else {
                self.request_latency.insert(
                    route.to_string(),
                    RouteLatency {
                        request_latency: vec![],
                        request_count: 0,
                        error_count: 1,
                        error_latency: latency,
                        status_codes: HashMap::new(),
                    },
                );
            }
        }

        // insert status code if it doesn't exist, otherwise increment
        // route should exist at this point
        let route_latency = self.request_latency.get_mut(route).unwrap();
        if let Some(status_code_count) = route_latency.status_codes.get_mut(&status_code) {
            *status_code_count += 1;
        } else {
            route_latency.status_codes.insert(status_code, 1);
        }
    }

    pub fn increment(&mut self, route: &str, latency: f64, status: &str, status_code: usize) {
        self.increment_request_count();
        self.update_route_latency(route, latency, status, status_code);
        self.increment_error_count(status);
    }

    pub fn collect_metrics(&self) -> ObservabilityMetrics {
        let latency_tuples = self
            .request_latency
            .iter()
            .map(|(route, route_latency)| {
                let mut latency_array = Array1::from_vec(
                    route_latency
                        .request_latency
                        .iter()
                        .map(|&x| n64(x))
                        .collect::<Vec<_>>(),
                );
                let qs = &[n64(0.05), n64(0.25), n64(0.5), n64(0.95), n64(0.99)];
                let quantiles = latency_array
                    .quantiles_mut(&Array1::from_vec(qs.to_vec()), &Nearest)
                    .unwrap();

                (
                    route,
                    RouteMetrics {
                        metrics: LatencyMetrics {
                            p5: quantiles[0].into(),
                            p25: quantiles[1].into(),
                            p50: quantiles[2].into(),
                            p95: quantiles[3].into(),
                            p99: quantiles[4].into(),
                        },
                        request_count: route_latency.request_count,
                        error_count: route_latency.error_count,
                        error_latency: route_latency.error_latency,
                        status_codes: route_latency.status_codes.clone(),
                    },
                )
            })
            .collect::<Vec<_>>();

        let route_metrics = latency_tuples
            .into_iter()
            .map(|(k, v)| (k.clone(), v))
            .collect::<HashMap<_, _>>();

        ObservabilityMetrics {
            request_count: self.request_count,
            error_count: self.error_count,
            route_metrics,
        }
    }

    pub fn reset_metrics(&mut self) {
        self.request_count = 0;
        self.error_count = 0;

        // Clear request latency for each route
        for (_, route_latency) in self.request_latency.iter_mut() {
            route_latency.request_latency = vec![];
            route_latency.request_count = 0;
            route_latency.error_count = 0;
            route_latency.error_latency = 0.0;
            route_latency.status_codes.clear();
        }
    }
}

#[pyclass]
#[derive(Clone, Debug)]
struct RouteMetrics {
    #[pyo3(get)]
    metrics: LatencyMetrics,

    #[pyo3(get)]
    request_count: i64,

    #[pyo3(get)]
    error_count: i64,

    #[pyo3(get)]
    error_latency: f64,

    #[pyo3(get)]
    status_codes: HashMap<usize, i64>,
}

#[pyclass]
#[derive(Debug)]
struct ObservabilityMetrics {
    #[pyo3(get)]
    request_count: i64,

    #[pyo3(get)]
    error_count: i64,

    #[pyo3(get)]
    route_metrics: HashMap<String, RouteMetrics>,
}

#[pymethods]
impl ObservabilityMetrics {
    #[new]
    pub fn new(
        request_count: i64,
        error_count: i64,
        route_metrics: HashMap<String, RouteMetrics>,
    ) -> Self {
        ObservabilityMetrics {
            request_count,
            error_count,
            route_metrics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn test_increment_request_count() {
        let mut observer = Observer::new();
        observer.increment_request_count();
        assert_eq!(observer.request_count, 1);
    }

    #[test]
    fn test_increment_error_count() {
        let mut observer = Observer::new();
        observer.increment_error_count("ERROR");
        assert_eq!(observer.error_count, 1);
        observer.increment_error_count("OK");
        assert_eq!(observer.error_count, 1);
    }

    #[test]
    fn test_update_route_latency() {
        let mut observer = Observer::new();
        observer.update_route_latency("/home", 100.0, "OK", 200);
        let sum_latency = observer
            .request_latency
            .get("/home")
            .unwrap()
            .request_latency
            .iter()
            .sum::<f64>();
        assert_eq!(sum_latency, 100.0);
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            1
        );

        observer.update_route_latency("/home", 50.0, "OK", 200);
        let sum_latency = observer
            .request_latency
            .get("/home")
            .unwrap()
            .request_latency
            .iter()
            .sum::<f64>();
        assert_eq!(sum_latency, 150.0);
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            2
        );

        observer.update_route_latency("/home", 50.0, "ERROR", 500);
        let sum_latency = observer
            .request_latency
            .get("/home")
            .unwrap()
            .request_latency
            .iter()
            .sum::<f64>();
        assert_eq!(sum_latency, 150.0);
        assert_eq!(
            observer.request_latency.get("/home").unwrap().error_latency,
            50.0
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            2
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().error_count,
            1
        );

        let status_codes = &observer.request_latency.get("/home").unwrap().status_codes;
        assert_eq!(status_codes.get(&200).unwrap(), &2);
        assert_eq!(status_codes.get(&500).unwrap(), &1);
    }

    #[test]
    fn test_collect_metrics() {
        //populate 3 routes with different latencies (n = 100)
        let mut observer = Observer::new();
        for i in 0..100 {
            // generate random latencies
            let num1 = rand::thread_rng().gen_range(0..100);
            let num2 = rand::thread_rng().gen_range(0..100);
            let num3 = rand::thread_rng().gen_range(0..100);
            observer.increment("/home", num1 as f64, "OK", 200);
            observer.increment("/home", 50.0 + i as f64, "ERROR", 404);
            observer.increment("/about", num2 as f64, "OK", 200);
            observer.increment("/contact", num3 as f64, "OK", 200);
        }

        let metrics = observer.collect_metrics();
        assert_eq!(metrics.request_count, 400);
        assert_eq!(metrics.error_count, 100);

        let route_metrics = metrics.route_metrics;

        let home_metrics = route_metrics.get("/home").unwrap();
        assert_eq!(home_metrics.request_count, 100);
        assert_eq!(home_metrics.error_count, 100);
    }

    #[test]
    fn test_increment() {
        let mut observer = Observer::new();
        observer.increment("/home", 100.0, "OK", 200);
        assert_eq!(observer.request_count, 1);
        assert_eq!(observer.error_count, 0);
        let sum_latency = observer
            .request_latency
            .get("/home")
            .unwrap()
            .request_latency
            .iter()
            .sum::<f64>();
        assert_eq!(sum_latency, 100.0);

        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            1
        );

        observer.increment("/home", 50.0, "ERROR", 500);
        assert_eq!(observer.request_count, 2);
        assert_eq!(observer.error_count, 1);
        let sum_latency = observer
            .request_latency
            .get("/home")
            .unwrap()
            .request_latency
            .iter()
            .sum::<f64>();
        assert_eq!(sum_latency, 100.0);

        assert_eq!(
            observer.request_latency.get("/home").unwrap().error_latency,
            50.0
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            1
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().error_count,
            1
        );
    }

    #[test]
    fn test_reset_metrics() {
        let mut observer = Observer::new();
        observer.increment("/home", 100.0, "OK", 200);
        observer.increment("/home", 50.0, "ERROR", 500);

        observer.reset_metrics();
        assert_eq!(observer.request_count, 0);
        assert_eq!(observer.error_count, 0);
        assert!(observer
            .request_latency
            .get("/home")
            .unwrap()
            .request_latency
            .is_empty());
        assert!(observer.request_latency.get("/home").unwrap().error_latency == 0.0);
        assert!(observer.request_latency.get("/home").unwrap().request_count == 0);
        assert!(observer.request_latency.get("/home").unwrap().error_count == 0);
    }
}
