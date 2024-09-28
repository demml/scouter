use pyo3::prelude::*;
use std::collections::HashMap;

#[pyclass]
#[derive(Clone, Debug)]
struct RouteLatency {
    #[pyo3(get)]
    request_latency: f64,

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
        let observer = Observer {
            request_count: 0,
            error_count: 0,
            request_latency: HashMap::new(),
        };

        observer
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
                route_latency.request_latency += latency;
                route_latency.request_count += 1;
            } else {
                self.request_latency.insert(
                    route.to_string(),
                    RouteLatency {
                        request_latency: latency,
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
                        request_latency: 0.0,
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
        ObservabilityMetrics::new(
            self.request_count,
            self.error_count,
            self.request_latency.clone(),
        )
    }

    pub fn reset_metrics(&mut self) {
        self.request_count = 0;
        self.error_count = 0;

        // Clear request latency for each route
        for (_, route_latency) in self.request_latency.iter_mut() {
            route_latency.request_latency = 0.0;
            route_latency.request_count = 0;
            route_latency.error_count = 0;
            route_latency.error_latency = 0.0;
            route_latency.status_codes.clear();
        }
    }
}

#[pyclass]
#[derive(Debug)]
struct ObservabilityMetrics {
    #[pyo3(get)]
    request_count: i64,

    #[pyo3(get)]
    error_count: i64,

    #[pyo3(get)]
    request_latency: HashMap<String, RouteLatency>,
}

#[pymethods]
impl ObservabilityMetrics {
    #[new]
    pub fn new(
        request_count: i64,
        error_count: i64,
        request_latency: HashMap<String, RouteLatency>,
    ) -> Self {
        ObservabilityMetrics {
            request_count,
            error_count,
            request_latency,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(
            observer
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency,
            100.0
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            1
        );

        observer.update_route_latency("/home", 50.0, "OK", 200);
        assert_eq!(
            observer
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency,
            150.0
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            2
        );

        observer.update_route_latency("/home", 50.0, "ERROR", 500);
        assert_eq!(
            observer
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency,
            150.0
        );
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
    fn test_increment() {
        let mut observer = Observer::new();
        observer.increment("/home", 100.0, "OK", 200);
        assert_eq!(observer.request_count, 1);
        assert_eq!(observer.error_count, 0);
        assert_eq!(
            observer
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency,
            100.0
        );
        assert_eq!(
            observer.request_latency.get("/home").unwrap().request_count,
            1
        );

        observer.increment("/home", 50.0, "ERROR", 500);
        assert_eq!(observer.request_count, 2);
        assert_eq!(observer.error_count, 1);
        assert_eq!(
            observer
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency,
            100.0
        );

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
    fn test_collect_metrics() {
        let mut observer = Observer::new();
        observer.increment("/home", 100.0, "OK", 202);
        observer.increment("/home", 50.0, "ERROR", 404);

        let metrics = observer.collect_metrics();
        assert_eq!(metrics.request_count, 2);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(
            metrics
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency,
            100.0
        );
        assert_eq!(
            metrics.request_latency.get("/home").unwrap().error_latency,
            50.0
        );
        assert_eq!(
            metrics.request_latency.get("/home").unwrap().request_count,
            1
        );
        assert_eq!(metrics.request_latency.get("/home").unwrap().error_count, 1);

        // check status codes
        let status_codes = &metrics.request_latency.get("/home").unwrap().status_codes;
        assert_eq!(status_codes.get(&202).unwrap(), &1);
        assert_eq!(status_codes.get(&404).unwrap(), &1);
    }

    #[test]
    fn test_reset_metrics() {
        let mut observer = Observer::new();
        observer.increment("/home", 100.0, "OK", 200);
        observer.increment("/home", 50.0, "ERROR", 500);

        observer.reset_metrics();
        assert_eq!(observer.request_count, 0);
        assert_eq!(observer.error_count, 0);
        assert!(
            observer
                .request_latency
                .get("/home")
                .unwrap()
                .request_latency
                == 0.0
        );
        assert!(observer.request_latency.get("/home").unwrap().error_latency == 0.0);
        assert!(observer.request_latency.get("/home").unwrap().request_count == 0);
        assert!(observer.request_latency.get("/home").unwrap().error_count == 0);
    }
}
