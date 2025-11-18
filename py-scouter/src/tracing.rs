use pyo3::prelude::*;
use scouter_client::{TraceBaggageRecord, TraceRecord, TraceSpanRecord};
use scouter_tracing::exporter::processor::BatchConfig;
use scouter_tracing::exporter::{
    GrpcSpanExporter, HttpSpanExporter, StdoutSpanExporter, TestSpanExporter,
};
use scouter_tracing::tracer::*;
use scouter_tracing::utils::{
    get_function_type, ExportConfig, FunctionType, GrpcConfig, HttpConfig, Protocol, SpanKind,
};

#[pymodule]
pub fn tracing(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<BaseTracer>()?;
    m.add_class::<ActiveSpan>()?;
    m.add_class::<SpanKind>()?;
    m.add_class::<FunctionType>()?;
    m.add_class::<ExportConfig>()?;
    m.add_class::<GrpcConfig>()?;
    m.add_class::<GrpcSpanExporter>()?;
    m.add_class::<HttpConfig>()?;
    m.add_class::<HttpSpanExporter>()?;
    m.add_class::<StdoutSpanExporter>()?;
    m.add_class::<Protocol>()?;
    m.add_class::<TraceRecord>()?;
    m.add_class::<TraceSpanRecord>()?;
    m.add_class::<TraceBaggageRecord>()?;
    m.add_class::<TestSpanExporter>()?;
    m.add_class::<BatchConfig>()?;
    m.add_function(wrap_pyfunction!(init_tracer, m)?)?;
    m.add_function(wrap_pyfunction!(flush_tracer, m)?)?;
    m.add_function(wrap_pyfunction!(get_function_type, m)?)?;
    m.add_function(wrap_pyfunction!(shutdown_tracer, m)?)?;
    Ok(())
}
