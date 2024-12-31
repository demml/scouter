use scouter_types::{RecordType, ServerRecord, ServerRecords, RouteMetrics, LatencyMetrics, ObservabilityMetrics};
use scouter_error::ObserverError;
use ndarray::Array1;
use ndarray_stats::interpolate::Nearest;
use ndarray_stats::Quantile1dExt;
use noisy_float::types::n64;
use pyo3::prelude::*;
use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
use std::collections::HashMap;
use tracing::{debug, error};

#[derive(Clone, Debug)]
struct RouteLatency {
    request_latency: Vec<f64>,
    request_count: i64,
    error_count: i64,
    error_latency: f64,
    status_codes: HashMap<usize, i64>,
}

#[pyclass]
#[derive(Clone, Debug)]
pub struct Observer {
    repository: String,
    name: String,
    version: String,
    request_count: i64,
    error_count: i64,
    request_latency: HashMap<String, RouteLatency>,
}

#[pymethods]
impl Observer {
    #[new]
    pub fn new(repository: String, name: String, version: String) -> Self {
        Observer {
            repository,
            name,
            version,
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
    ) -> Result<(), ObserverError> {
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
        let route_latency = self
            .request_latency
            .get_mut(route)
            .ok_or(ObserverError::RouteNotFound(route.to_string()))?;
        if let Some(status_code_count) = route_latency.status_codes.get_mut(&status_code) {
            *status_code_count += 1;
        } else {
            route_latency.status_codes.insert(status_code, 1);
        }

        Ok(())
    }

    pub fn increment(
        &mut self,
        route: &str,
        latency: f64,
        status_code: usize,
    ) -> Result<(), ObserverError> {
        let status = if (200..400).contains(&status_code) {
            "OK"
        } else {
            "ERROR"
        };

        self.increment_request_count();
        self.update_route_latency(route, latency, status, status_code)
            .map_err(|e| {
                error!("Failed to update route latency: {:?}", e);
            })
            .ok();
        self.increment_error_count(status);

        Ok(())
    }

    pub fn collect_metrics(&self) -> Result<Option<ServerRecords>, ObserverError> {
        if self.request_count == 0 {
            return Ok(None);
        }

        debug!("Collecting metrics: {:?}", self.request_latency);

        let route_metrics = self
            .request_latency
            .clone()
            .into_par_iter()
            .map(|(route, route_latency)| {
                let mut latency_array = Array1::from_vec(
                    route_latency
                        .request_latency
                        .iter()
                        .map(|&x| n64(x))
                        .collect::<Vec<_>>(),
                );
                let qs = &[n64(0.05), n64(0.25), n64(0.5), n64(0.95), n64(0.99)];
                let quantiles =
                    latency_array.quantiles_mut(&Array1::from_vec(qs.to_vec()), &Nearest);

                match quantiles {
                    Ok(quantiles) => Ok(RouteMetrics {
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
                        route_name: route,
                    }),
                    // its ok if route fails, but we want to know why
                    Err(e) => Err(ObserverError::QuantileError(e.to_string())),
                }
            })
            .collect::<Vec<Result<RouteMetrics, ObserverError>>>();

        // check if any route failed (log it and filter it out)
        let route_metrics = route_metrics
            .into_iter()
            .filter_map(|x| match x {
                Ok(route_metrics) => Some(route_metrics),
                Err(e) => {
                    debug!("Failed to collect metrics for route: {:?}", e);
                    None
                }
            })
            .collect::<Vec<RouteMetrics>>();

        // check if there are no metrics and exit early
        if route_metrics.is_empty() {
            return Ok(None);
        }

        let record = ServerRecord::Observability {
            record: ObservabilityMetrics {
                repository: self.repository.clone(),
                name: self.name.clone(),
                version: self.version.clone(),
                request_count: self.request_count,
                error_count: self.error_count,
                route_metrics,
                record_type: RecordType::Observability,
            },
        };

        Ok(Some(ServerRecords {
            record_type: RecordType::Observability,
            records: vec![record],
        }))
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    const REPOSITORY: &str = "test";
    const NAME: &str = "test";
    const VERSION: &str = "test";

    #[test]
    fn test_increment_request_count() {
        let mut observer = Observer::new(
            REPOSITORY.to_string(),
            NAME.to_string(),
            VERSION.to_string(),
        );
        observer.increment_request_count();
        assert_eq!(observer.request_count, 1);
    }

    #[test]
    fn test_increment_error_count() {
        let mut observer = Observer::new(
            REPOSITORY.to_string(),
            NAME.to_string(),
            VERSION.to_string(),
        );
        observer.increment_error_count("ERROR");
        assert_eq!(observer.error_count, 1);
        observer.increment_error_count("OK");
        assert_eq!(observer.error_count, 1);
    }

    #[test]
    fn test_update_route_latency() {
        let mut observer = Observer::new(
            REPOSITORY.to_string(),
            NAME.to_string(),
            VERSION.to_string(),
        );
        observer
            .update_route_latency("/home", 100.0, "OK", 200)
            .unwrap();
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

        observer
            .update_route_latency("/home", 50.0, "OK", 200)
            .unwrap();
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

        observer
            .update_route_latency("/home", 50.0, "ERROR", 500)
            .unwrap();
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
        let mut observer = Observer::new(
            REPOSITORY.to_string(),
            NAME.to_string(),
            VERSION.to_string(),
        );
        for i in 0..100 {
            // generate random latencies
            let num1 = rand::thread_rng().gen_range(0..100);
            let num2 = rand::thread_rng().gen_range(0..100);
            let num3 = rand::thread_rng().gen_range(0..100);
            observer.increment("/home", num1 as f64, 200).unwrap();
            observer.increment("/home", 50.0 + i as f64, 404).unwrap();
            observer.increment("/about", num2 as f64, 200).unwrap();
            observer.increment("/contact", num3 as f64, 200).unwrap();
        }

        let metrics = observer.collect_metrics().unwrap().unwrap();
        metrics.model_dump_json();
        metrics.__str__();

        // get record

        let metrics = metrics.records[0].clone();

        // check observability metrics
        let record = match metrics {
            ServerRecord::Observability { record } => record,
            _ => panic!("Expected observability record"),
        };

        assert_eq!(record.request_count, 400);
        assert_eq!(record.error_count, 100);
        assert_eq!(record.repository, REPOSITORY);
        assert_eq!(record.name, NAME);
        assert_eq!(record.version, VERSION);

        let route_metrics = record.route_metrics;

        // check route metrics. Filter to get home route metrics
        let home_metrics = route_metrics
            .iter()
            .find(|x| x.route_name == "/home")
            .unwrap();

        assert_eq!(home_metrics.request_count, 100);
        assert_eq!(home_metrics.error_count, 100);
    }

    #[test]
    fn test_increment() {
        let mut observer = Observer::new(
            REPOSITORY.to_string(),
            NAME.to_string(),
            VERSION.to_string(),
        );
        observer.increment("/home", 100.0, 200).unwrap();
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

        observer.increment("/home", 50.0, 500).unwrap();
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
        let mut observer = Observer::new(
            REPOSITORY.to_string(),
            NAME.to_string(),
            VERSION.to_string(),
        );
        observer.increment("/home", 100.0, 200).unwrap();
        observer.increment("/home", 50.0, 500).unwrap();

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
