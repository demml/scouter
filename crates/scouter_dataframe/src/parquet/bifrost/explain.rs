use crate::parquet::bifrost::query::QueryExecutionMetadata;
use datafusion::logical_expr::LogicalPlan;
use datafusion::physical_plan::displayable;
use datafusion::physical_plan::ExecutionPlan;

/// Strip object-store URIs and absolute file paths from DataFusion plan text.
///
/// DataFusion's `displayable(plan).indent(true)` and `display_indent()` embed
/// real storage paths (S3 URIs, GCS bucket prefixes, local absolute paths).
/// This function replaces them with `<storage-path>` before the text is returned
/// to API callers, preventing storage topology leaks.
pub fn sanitize_plan_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for token in text.split_inclusive(|c: char| {
        c.is_whitespace() || c == ',' || c == '[' || c == ']' || c == '(' || c == ')'
    }) {
        // Trim trailing delimiters to check just the token body
        let trimmed = token.trim_end_matches(|c: char| {
            c.is_whitespace() || c == ',' || c == '[' || c == ']' || c == '(' || c == ')'
        });
        let suffix = &token[trimmed.len()..];

        let is_storage_uri = trimmed.starts_with("s3://")
            || trimmed.starts_with("gs://")
            || trimmed.starts_with("az://")
            || trimmed.starts_with("abfss://")
            || trimmed.starts_with("file://")
            || (trimmed.starts_with('/')
                && (trimmed.ends_with(".parquet") || trimmed.contains("/datasets/")));

        if is_storage_uri && !trimmed.is_empty() {
            result.push_str("<storage-path>");
            result.push_str(suffix);
        } else {
            result.push_str(token);
        }
    }
    result
}

/// Structured representation of a query plan node for UI rendering.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanNode {
    pub node_type: String,
    pub description: String,
    pub fields: Vec<PlanNodeField>,
    pub children: Vec<PlanNode>,
    pub metrics: Option<PlanNodeMetrics>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanNodeField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PlanNodeMetrics {
    pub output_rows: Option<u64>,
    pub elapsed_ms: Option<f64>,
    pub bytes_scanned: Option<u64>,
    pub spill_bytes: Option<u64>,
}

/// Result of an EXPLAIN query.
pub struct ExplainResult {
    pub logical_plan: PlanNode,
    pub physical_plan: PlanNode,
    pub logical_plan_text: String,
    pub physical_plan_text: String,
    pub execution_metadata: Option<QueryExecutionMetadata>,
}

/// Recursively convert a DataFusion LogicalPlan into a PlanNode tree.
pub fn logical_plan_to_tree(plan: &LogicalPlan) -> PlanNode {
    let node_type = format!("{:?}", plan)
        .split('(')
        .next()
        .unwrap_or("Unknown")
        .to_string();
    let description = plan.display().to_string();

    let children: Vec<PlanNode> = plan.inputs().iter().map(|p| logical_plan_to_tree(p)).collect();

    PlanNode {
        node_type,
        description,
        fields: vec![],
        children,
        metrics: None,
    }
}

/// Recursively convert a DataFusion ExecutionPlan into a PlanNode tree.
pub fn physical_plan_to_tree(plan: &dyn ExecutionPlan) -> PlanNode {
    let node_type = plan.name().to_string();
    let description = displayable(plan).one_line().to_string();

    let children: Vec<PlanNode> = plan
        .children()
        .iter()
        .map(|p| physical_plan_to_tree(p.as_ref()))
        .collect();

    // Extract metrics if available (populated after ANALYZE execution)
    let metrics = plan.metrics().map(|m| {
        let output_rows = m.output_rows();
        let elapsed = m.elapsed_compute();
        PlanNodeMetrics {
            output_rows: output_rows.map(|r| r as u64),
            elapsed_ms: elapsed.map(|ns| ns as f64 / 1_000_000.0),
            bytes_scanned: m
                .sum_by_name("bytes_scanned")
                .map(|v| v.as_usize() as u64),
            spill_bytes: m
                .sum_by_name("spill_count")
                .map(|v| v.as_usize() as u64),
        }
    });

    PlanNode {
        node_type,
        description,
        fields: vec![],
        children,
        metrics,
    }
}
