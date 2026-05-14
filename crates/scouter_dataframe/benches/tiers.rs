#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum BenchTier {
    Tier0 = 0,
    Tier1 = 1,
    Tier2 = 2,
}

impl BenchTier {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Tier0),
            1 => Some(Self::Tier1),
            2 => Some(Self::Tier2),
            _ => None,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BenchRegistration {
    pub bench_binary: &'static str,
    pub group_name: &'static str,
    pub tier: BenchTier,
    pub runtime_budget_secs: u64,
    pub fixture_rows: u64,
    pub fixture_spans: u64,
    pub storage_profile: &'static str,
    pub scenario_class: &'static str,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SpanMetric {
    pub count: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub sum_us: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObjectStoreCountSnapshot {
    pub list: u64,
    pub list_with_delimiter: u64,
    pub head: u64,
    pub get: u64,
    pub get_range: u64,
    pub put: u64,
    pub delete: u64,
    pub copy: u64,
    pub bytes: u64,
}

impl ObjectStoreCountSnapshot {
    pub fn total_operations(&self) -> u64 {
        self.list
            + self.list_with_delimiter
            + self.head
            + self.get
            + self.get_range
            + self.put
            + self.delete
            + self.copy
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchArtifact {
    pub commit: String,
    pub bench_group: String,
    pub tier: u8,
    pub blocking: bool,
    pub scenario_class: String,
    pub runtime_budget_secs: u64,
    pub actual_runtime_secs: f64,
    pub fixture_rows: u64,
    pub fixture_spans: u64,
    pub storage_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_entrypoint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_rows: Option<u64>,
    pub spans: BTreeMap<String, SpanMetric>,
    pub object_store_counts: ObjectStoreCountSnapshot,
    #[serde(default)]
    pub refresh_on_request_path_total: u64,
}

pub const END_TO_END_SPAN: &str = "bench.query.end_to_end";
pub const DF_COLLECT_SPAN: &str = "df.collect";
pub const DELTA_SNAPSHOT_REFRESH_SPAN: &str = "delta.snapshot.refresh";

pub const P1_LOCAL_NVME: &str = "P1_local_nvme";
pub const P2_OBJECT_WARM: &str = "P2_object_warm";
pub const P2_OBJECT_COLD: &str = "P2_object_cold";

pub const REGISTRY: &[BenchRegistration] = &[
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "t0_cold_query_smoke",
        tier: BenchTier::Tier0,
        runtime_budget_secs: 120,
        fixture_rows: 10_080,
        fixture_spans: 10_080,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "cold_query",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "t0_refresh_origin_sentinel",
        tier: BenchTier::Tier0,
        runtime_budget_secs: 30,
        fixture_rows: 0,
        fixture_spans: 0,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "refresh_origin_sentinel",
    },
    BenchRegistration {
        bench_binary: "hot_path_bench",
        group_name: "t0_hot_path_cold_query_smoke",
        tier: BenchTier::Tier0,
        runtime_budget_secs: 120,
        fixture_rows: 10_000,
        fixture_spans: 10_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "cold_query",
    },
    BenchRegistration {
        bench_binary: "dataset_benchmark",
        group_name: "t0_bifrost_smoke",
        tier: BenchTier::Tier0,
        runtime_budget_secs: 120,
        fixture_rows: 1_000,
        fixture_spans: 0,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "bifrost_smoke",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "write_throughput",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 50_000,
        fixture_spans: 50_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "write_throughput",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "concurrent_writes",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 200,
        fixture_spans: 200,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "write_throughput",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "query_performance",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 100_000,
        fixture_spans: 100_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "cold_query",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "sustained_load",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 1_000,
        fixture_spans: 1_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "write_throughput",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "query_at_scale",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 3600,
        fixture_rows: 100_000,
        fixture_spans: 100_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "cold_query",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 10_080,
        fixture_spans: 10_080,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "cold_query",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "bounded_full",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 10_080,
        fixture_spans: 10_080,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "start_time_hint_only",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 10_080,
        fixture_spans: 10_080,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "cache_hit",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 1800,
        fixture_rows: 10_080,
        fixture_spans: 10_080,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "at_scale_1m",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 7200,
        fixture_rows: 1_000_000,
        fixture_spans: 1_000_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "trace_service_benchmark",
        group_name: "at_scale_10m",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 21_600,
        fixture_rows: 10_000_000,
        fixture_spans: 10_000_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "dataset_benchmark",
        group_name: "dataset_write",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 3600,
        fixture_rows: 10_000,
        fixture_spans: 0,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "dataset_write",
    },
    BenchRegistration {
        bench_binary: "dataset_benchmark",
        group_name: "dataset_query",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 3600,
        fixture_rows: 10_000,
        fixture_spans: 0,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "bifrost_query",
    },
    BenchRegistration {
        bench_binary: "hot_path_bench",
        group_name: "trace_hot_paths_1m",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 7200,
        fixture_rows: 1_000_000,
        fixture_spans: 1_000_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "selective_lookup",
    },
    BenchRegistration {
        bench_binary: "hot_path_bench",
        group_name: "metrics_hot_paths_1m",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 7200,
        fixture_rows: 1_000_000,
        fixture_spans: 1_000_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "dashboard",
    },
    BenchRegistration {
        bench_binary: "planner_bench",
        group_name: "planner_queries",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 3600,
        fixture_rows: 1_000_000,
        fixture_spans: 1_000_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "planner",
    },
    BenchRegistration {
        bench_binary: "session_config_bench",
        group_name: "session_config_bench",
        tier: BenchTier::Tier1,
        runtime_budget_secs: 3600,
        fixture_rows: 100_000,
        fixture_spans: 100_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "session_config",
    },
    BenchRegistration {
        bench_binary: "stress_test",
        group_name: "stress_test",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 21_600,
        fixture_rows: 1_000_000,
        fixture_spans: 1_000_000,
        storage_profile: P1_LOCAL_NVME,
        scenario_class: "soak",
    },
    BenchRegistration {
        bench_binary: "cloud_backed_runs",
        group_name: "p2_object_warm",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 21_600,
        fixture_rows: 0,
        fixture_spans: 0,
        storage_profile: P2_OBJECT_WARM,
        scenario_class: "cloud_object_store",
    },
    BenchRegistration {
        bench_binary: "cloud_backed_runs",
        group_name: "p2_object_cold",
        tier: BenchTier::Tier2,
        runtime_budget_secs: 21_600,
        fixture_rows: 0,
        fixture_spans: 0,
        storage_profile: P2_OBJECT_COLD,
        scenario_class: "cloud_object_store",
    },
];

pub fn current_tier() -> BenchTier {
    env::var("SCOUTER_BENCH_TIER")
        .ok()
        .and_then(|value| value.parse::<u8>().ok())
        .and_then(BenchTier::from_u8)
        .unwrap_or(BenchTier::Tier0)
}

pub fn tier_for(group_name: &str) -> BenchTier {
    REGISTRY
        .iter()
        .find(|entry| entry.group_name == group_name)
        .map(|entry| entry.tier)
        .unwrap_or(BenchTier::Tier1)
}

pub fn registration_for(
    bench_binary: &str,
    group_name: &str,
) -> Option<&'static BenchRegistration> {
    REGISTRY
        .iter()
        .find(|entry| entry.bench_binary == bench_binary && entry.group_name == group_name)
}

pub fn registration_or_default(
    bench_binary: &'static str,
    group_name: &'static str,
) -> BenchRegistration {
    registration_for(bench_binary, group_name)
        .copied()
        .unwrap_or(BenchRegistration {
            bench_binary,
            group_name,
            tier: BenchTier::Tier1,
            runtime_budget_secs: 3600,
            fixture_rows: 0,
            fixture_spans: 0,
            storage_profile: P1_LOCAL_NVME,
            scenario_class: "unregistered",
        })
}

pub fn tier_guard(group_name: &'static str) -> bool {
    guard_registration(&registration_or_default("unknown", group_name))
}

pub fn tier_guard_for(bench_binary: &'static str, group_name: &'static str) -> bool {
    guard_registration(&registration_or_default(bench_binary, group_name))
}

fn guard_registration(registration: &BenchRegistration) -> bool {
    let requested = current_tier();
    if requested == registration.tier {
        return true;
    }

    eprintln!(
        "skipping {}::{}: registered tier {} does not match SCOUTER_BENCH_TIER={}",
        registration.bench_binary,
        registration.group_name,
        registration.tier.as_u8(),
        requested.as_u8()
    );
    false
}

pub fn filter_for(tier: BenchTier, bench_binary: &str) -> Result<String, String> {
    let groups: Vec<&str> = REGISTRY
        .iter()
        .filter(|entry| entry.bench_binary == bench_binary && entry.tier == tier)
        .map(|entry| entry.group_name)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    if groups.is_empty() {
        return Err(format!(
            "no tier {} groups registered for {bench_binary}",
            tier.as_u8()
        ));
    }

    Ok(format!("^({})$", groups.join("|")))
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn run_metrics_dir() -> PathBuf {
    repo_root().join("target").join("bench_metrics")
}

fn baseline_metrics_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("bench_metrics")
}

fn read_artifact(path: &Path) -> Result<BenchArtifact, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    serde_json::from_str(&contents)
        .map_err(|err| format!("failed to parse {}: {err}", path.display()))
}

fn regression_percent(run: u64, baseline: u64) -> Option<f64> {
    if baseline == 0 {
        return (run > 0).then_some(f64::INFINITY);
    }
    (run > baseline).then(|| ((run - baseline) as f64 / baseline as f64) * 100.0)
}

fn compare_count(
    failures: &mut Vec<String>,
    label: &str,
    run: u64,
    baseline: u64,
    tier: BenchTier,
) {
    if let Some(percent) = regression_percent(run, baseline)
        && percent > 10.0
        && tier == BenchTier::Tier0
    {
        failures.push(format!(
            "{label} regressed by {percent:.1}%: run={run}, baseline={baseline}"
        ));
    }
}

fn end_to_end_count(artifact: &BenchArtifact) -> Option<u64> {
    artifact
        .spans
        .get(END_TO_END_SPAN)
        .map(|metric| metric.count)
}

fn compare_rate(
    failures: &mut Vec<String>,
    label: &str,
    run: u64,
    run_denominator: u64,
    baseline: u64,
    baseline_denominator: u64,
    tier: BenchTier,
) {
    if run_denominator == 0 || baseline_denominator == 0 {
        compare_count(failures, label, run, baseline, tier);
        return;
    }

    let run_rate = run as f64 / run_denominator as f64;
    let baseline_rate = baseline as f64 / baseline_denominator as f64;
    if baseline_rate == 0.0 {
        if run_rate > 0.0 && tier == BenchTier::Tier0 {
            failures.push(format!(
                "{label} rate regressed from zero: run={run_rate:.4}, baseline=0.0000"
            ));
        }
        return;
    }

    let percent = ((run_rate - baseline_rate) / baseline_rate) * 100.0;
    if percent > 10.0 && tier == BenchTier::Tier0 {
        failures.push(format!(
            "{label} rate regressed by {percent:.1}%: run={run_rate:.4}, baseline={baseline_rate:.4}"
        ));
    }
}

fn compare_span(
    failures: &mut Vec<String>,
    name: &str,
    run: &BenchArtifact,
    baseline: &BenchArtifact,
    tier: BenchTier,
) {
    let Some(run_span) = run.spans.get(name) else {
        return;
    };
    let Some(base_span) = baseline.spans.get(name) else {
        return;
    };
    if run_span.count < 10 || base_span.count < 10 {
        return;
    }
    compare_count(
        failures,
        &format!("spans[{name}].p95_us"),
        run_span.p95_us,
        base_span.p95_us,
        tier,
    );
}

fn compare_artifacts(
    failures: &mut Vec<String>,
    run: &BenchArtifact,
    baseline: &BenchArtifact,
    tier: BenchTier,
) {
    if baseline.spans.contains_key(END_TO_END_SPAN) && !run.spans.contains_key(END_TO_END_SPAN) {
        failures.push(format!(
            "{} missing primary {END_TO_END_SPAN} metric present in baseline",
            run.bench_group
        ));
    }

    compare_span(failures, END_TO_END_SPAN, run, baseline, tier);

    let run_end_to_end_count = end_to_end_count(run).unwrap_or(0);
    let baseline_end_to_end_count = end_to_end_count(baseline).unwrap_or(0);
    compare_span(failures, DELTA_SNAPSHOT_REFRESH_SPAN, run, baseline, tier);
    compare_span(failures, DF_COLLECT_SPAN, run, baseline, tier);
    compare_rate(
        failures,
        "object_store_counts.get_range",
        run.object_store_counts.get_range,
        run_end_to_end_count,
        baseline.object_store_counts.get_range,
        baseline_end_to_end_count,
        tier,
    );
    compare_rate(
        failures,
        "object_store_counts.head",
        run.object_store_counts.head,
        run_end_to_end_count,
        baseline.object_store_counts.head,
        baseline_end_to_end_count,
        tier,
    );
    compare_rate(
        failures,
        "object_store_counts.list",
        run.object_store_counts.list,
        run_end_to_end_count,
        baseline.object_store_counts.list,
        baseline_end_to_end_count,
        tier,
    );
}

fn compare_artifact(run_path: &Path, requested_tier: BenchTier) -> Result<Vec<String>, String> {
    let run = read_artifact(run_path)?;
    let Some(run_tier) = BenchTier::from_u8(run.tier) else {
        return Err(format!(
            "{} has invalid tier {}",
            run_path.display(),
            run.tier
        ));
    };

    if run_tier != requested_tier {
        return Ok(vec![format!(
            "skipped {} because artifact tier {} does not match requested tier {}",
            run.bench_group,
            run.tier,
            requested_tier.as_u8()
        )]);
    }

    if run.blocking != (run_tier == BenchTier::Tier0) {
        return Err(format!(
            "{} has blocking={} but tier={}",
            run.bench_group, run.blocking, run.tier
        ));
    }

    let mut failures = Vec::new();
    let mut notes = Vec::new();

    validate_artifact_execution(&run, run_tier, &mut failures);

    if run.actual_runtime_secs > run.runtime_budget_secs as f64 && run_tier == BenchTier::Tier0 {
        failures.push(format!(
            "{} exceeded runtime budget: {:.2}s > {}s",
            run.bench_group, run.actual_runtime_secs, run.runtime_budget_secs
        ));
    }

    if run.refresh_on_request_path_total > 0 && run_tier == BenchTier::Tier0 {
        failures.push(format!(
            "{} recorded refresh-on-request count {}",
            run.bench_group, run.refresh_on_request_path_total
        ));
    }

    let baseline_path = baseline_metrics_dir().join(format!("{}.json", run.bench_group));
    if !baseline_path.exists() {
        notes.push(format!(
            "no committed baseline for {}; comparator did not hard-fail",
            run.bench_group
        ));
    } else {
        let baseline = read_artifact(&baseline_path)?;
        compare_artifacts(&mut failures, &run, &baseline, run_tier);
    }

    if run_tier != BenchTier::Tier0 && !failures.is_empty() {
        notes.push(format!(
            "{} is tier {}; comparator refuses to hard-fail non-Tier-0 artifacts",
            run.bench_group, run.tier
        ));
        failures.clear();
    }

    if failures.is_empty() {
        Ok(notes)
    } else {
        Err(failures.join("\n"))
    }
}

fn validate_artifact_execution(
    run: &BenchArtifact,
    run_tier: BenchTier,
    failures: &mut Vec<String>,
) {
    if run_tier != BenchTier::Tier0 {
        return;
    }

    let Some(registration) = REGISTRY
        .iter()
        .find(|entry| entry.group_name == run.bench_group && entry.tier == BenchTier::Tier0)
    else {
        failures.push(format!(
            "{} is not registered as a Tier 0 benchmark group",
            run.bench_group
        ));
        return;
    };

    if run.fixture_rows != registration.fixture_rows {
        failures.push(format!(
            "{} reported fixture_rows={} but registry expects {}",
            run.bench_group, run.fixture_rows, registration.fixture_rows
        ));
    }

    if run.fixture_spans != registration.fixture_spans {
        failures.push(format!(
            "{} reported fixture_spans={} but registry expects {}",
            run.bench_group, run.fixture_spans, registration.fixture_spans
        ));
    }

    if run.scenario_class != registration.scenario_class {
        failures.push(format!(
            "{} reported scenario_class={} but registry expects {}",
            run.bench_group, run.scenario_class, registration.scenario_class
        ));
    }

    if run.storage_profile != registration.storage_profile {
        failures.push(format!(
            "{} reported storage_profile={} but registry expects {}",
            run.bench_group, run.storage_profile, registration.storage_profile
        ));
    }

    if run.bench_group == "t0_refresh_origin_sentinel" {
        if run.refresh_on_request_path_total != 0 {
            failures.push(format!(
                "{} recorded refresh_on_request_path_total={}",
                run.bench_group, run.refresh_on_request_path_total
            ));
        }
        return;
    }

    if run.query_entrypoint.is_none() {
        failures.push(format!("{} missing query_entrypoint", run.bench_group));
    }

    match run.result_rows {
        Some(rows) if rows > 0 => {}
        _ => failures.push(format!("{} missing or zero result_rows", run.bench_group)),
    }

    match run.spans.get(END_TO_END_SPAN) {
        Some(metric) if metric.count >= 10 && metric.sum_us > 0 => {}
        Some(metric) => failures.push(format!(
            "{} did not record a measured {END_TO_END_SPAN} workload: count={}, sum_us={}",
            run.bench_group, metric.count, metric.sum_us
        )),
        None => failures.push(format!(
            "{} did not record required {END_TO_END_SPAN} span metrics",
            run.bench_group
        )),
    }

    if run.actual_runtime_secs <= 0.0 {
        failures.push(format!(
            "{} reported non-positive actual_runtime_secs={}",
            run.bench_group, run.actual_runtime_secs
        ));
    }

    if run.object_store_counts.total_operations() == 0 {
        failures.push(format!(
            "{} did not record any object-store operations; this is a smoke artifact, not a baseline",
            run.bench_group
        ));
    }
}

fn compare_requested_tier(tier: BenchTier) -> Result<(), String> {
    let dir = run_metrics_dir();
    if !dir.exists() {
        if tier == BenchTier::Tier0 {
            return Err(format!(
                "missing Tier 0 bench artifacts directory {}; run make bench.core before comparing",
                dir.display()
            ));
        }
        return Ok(());
    }

    let mut notes = Vec::new();
    let mut compared = 0usize;
    let mut seen = BTreeSet::new();
    for entry in fs::read_dir(&dir).map_err(|err| format!("failed to read {dir:?}: {err}"))? {
        let entry = entry.map_err(|err| format!("failed to read dir entry: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !REGISTRY
            .iter()
            .any(|entry| entry.group_name == stem && entry.tier == tier)
        {
            notes.push(format!(
                "skipped {} because it is not registered for tier {}",
                path.display(),
                tier.as_u8()
            ));
            continue;
        }
        compared += 1;
        seen.insert(stem.to_string());
        match compare_artifact(&path, tier) {
            Ok(artifact_notes) => notes.extend(artifact_notes),
            Err(err) if tier != BenchTier::Tier0 => notes.push(format!(
                "advisory tier ignored comparator error for {}: {err}",
                path.display()
            )),
            Err(err) => return Err(err),
        }
    }

    if tier == BenchTier::Tier0 {
        let missing = REGISTRY
            .iter()
            .filter(|entry| entry.tier == BenchTier::Tier0)
            .map(|entry| entry.group_name)
            .filter(|group_name| !seen.contains(*group_name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(format!(
                "missing Tier 0 bench artifact(s): {}",
                missing.join(", ")
            ));
        }
    }

    for note in notes {
        eprintln!("{note}");
    }
    eprintln!(
        "bench comparator examined {compared} artifact(s) for tier {}",
        tier.as_u8()
    );
    Ok(())
}

fn parse_tier_arg(args: &[String]) -> Result<BenchTier, String> {
    let tier = args
        .windows(2)
        .find_map(|window| (window[0] == "--tier").then(|| window[1].clone()))
        .unwrap_or_else(|| "0".to_string());
    tier.parse::<u8>()
        .ok()
        .and_then(BenchTier::from_u8)
        .ok_or_else(|| format!("invalid --tier value {tier}; expected 0, 1, or 2"))
}

fn parse_bench_arg(args: &[String]) -> Result<String, String> {
    args.windows(2)
        .find_map(|window| (window[0] == "--bench").then(|| window[1].clone()))
        .ok_or_else(|| "--bench is required".to_string())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let invoked = Path::new(
        args.first()
            .map(String::as_str)
            .unwrap_or("bench_tier_filter"),
    )
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or("bench_tier_filter");

    let result = if invoked.contains("bench_compare") {
        parse_tier_arg(&args).and_then(compare_requested_tier)
    } else {
        parse_tier_arg(&args).and_then(|tier| {
            let bench = parse_bench_arg(&args)?;
            filter_for(tier, &bench).map(|filter| {
                println!("{filter}");
            })
        })
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
}

#[cfg(test)]
#[allow(unused_imports)]
mod tests {
    use super::*;

    fn tier0_artifact(group_name: &str) -> BenchArtifact {
        let registration = REGISTRY
            .iter()
            .find(|entry| entry.group_name == group_name && entry.tier == BenchTier::Tier0)
            .unwrap();
        let mut spans = BTreeMap::new();
        spans.insert(
            END_TO_END_SPAN.to_string(),
            SpanMetric {
                count: 10,
                p50_us: 100,
                p95_us: 150,
                p99_us: 200,
                sum_us: 1_200,
            },
        );
        BenchArtifact {
            commit: "test".to_string(),
            bench_group: group_name.to_string(),
            tier: BenchTier::Tier0.as_u8(),
            blocking: true,
            scenario_class: registration.scenario_class.to_string(),
            runtime_budget_secs: registration.runtime_budget_secs,
            actual_runtime_secs: 1.0,
            fixture_rows: registration.fixture_rows,
            fixture_spans: registration.fixture_spans,
            storage_profile: registration.storage_profile.to_string(),
            query_entrypoint: Some("test_entrypoint".to_string()),
            result_rows: Some(10),
            spans,
            object_store_counts: ObjectStoreCountSnapshot {
                list: 1,
                ..ObjectStoreCountSnapshot::default()
            },
            refresh_on_request_path_total: 0,
        }
    }

    #[test]
    fn tier0_filter_is_exactly_anchored() {
        let filter = filter_for(BenchTier::Tier0, "trace_service_benchmark").unwrap();
        assert_eq!(filter, "^(t0_cold_query_smoke|t0_refresh_origin_sentinel)$");
    }

    #[test]
    fn missing_tier_filter_refuses_empty_output() {
        let err = filter_for(BenchTier::Tier0, "stress_test").unwrap_err();
        assert!(err.contains("no tier 0 groups"));
    }

    #[test]
    fn unknown_groups_default_to_tier1() {
        assert_eq!(tier_for("not_registered"), BenchTier::Tier1);
    }

    #[test]
    fn tier0_artifact_rejects_single_end_to_end_probe() {
        let mut artifact = tier0_artifact("t0_hot_path_cold_query_smoke");
        artifact.spans.insert(
            END_TO_END_SPAN.to_string(),
            SpanMetric {
                count: 1,
                p50_us: 5,
                p95_us: 5,
                p99_us: 5,
                sum_us: 5,
            },
        );

        let mut failures = Vec::new();
        validate_artifact_execution(&artifact, BenchTier::Tier0, &mut failures);

        assert!(
            failures.iter().any(|failure| failure
                .contains("did not record a measured bench.query.end_to_end workload")),
            "{failures:?}"
        );
    }

    #[test]
    fn tier0_artifact_rejects_missing_bench_query_end_to_end() {
        let mut artifact = tier0_artifact("t0_hot_path_cold_query_smoke");
        artifact.spans.remove(END_TO_END_SPAN);
        let mut failures = Vec::new();
        validate_artifact_execution(&artifact, BenchTier::Tier0, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("bench.query.end_to_end")),
            "{failures:?}"
        );
    }

    #[test]
    fn tier0_artifact_rejects_missing_query_entrypoint() {
        let mut artifact = tier0_artifact("t0_hot_path_cold_query_smoke");
        artifact.query_entrypoint = None;
        let mut failures = Vec::new();
        validate_artifact_execution(&artifact, BenchTier::Tier0, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("query_entrypoint")),
            "{failures:?}"
        );
    }

    #[test]
    fn tier0_artifact_rejects_missing_result_rows() {
        let mut artifact = tier0_artifact("t0_hot_path_cold_query_smoke");
        artifact.result_rows = None;
        let mut failures = Vec::new();
        validate_artifact_execution(&artifact, BenchTier::Tier0, &mut failures);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("result_rows")),
            "{failures:?}"
        );
    }

    #[test]
    fn tier0_artifact_rejects_missing_object_store_counts() {
        let mut artifact = tier0_artifact("t0_hot_path_cold_query_smoke");
        artifact.object_store_counts = ObjectStoreCountSnapshot::default();

        let mut failures = Vec::new();
        validate_artifact_execution(&artifact, BenchTier::Tier0, &mut failures);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("did not record any object-store operations")),
            "{failures:?}"
        );
    }

    #[test]
    fn tier0_refresh_origin_sentinel_allows_zero_workload_metrics() {
        let mut artifact = tier0_artifact("t0_refresh_origin_sentinel");
        artifact.spans.clear();
        artifact.object_store_counts = ObjectStoreCountSnapshot::default();

        let mut failures = Vec::new();
        validate_artifact_execution(&artifact, BenchTier::Tier0, &mut failures);

        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn tier0_end_to_end_regression_fails_even_if_df_collect_improves() {
        let mut baseline = tier0_artifact("t0_hot_path_cold_query_smoke");
        baseline.spans.insert(
            END_TO_END_SPAN.to_string(),
            SpanMetric {
                count: 10,
                p50_us: 100,
                p95_us: 100,
                p99_us: 100,
                sum_us: 1_000,
            },
        );
        baseline.spans.insert(
            DF_COLLECT_SPAN.to_string(),
            SpanMetric {
                count: 10,
                p50_us: 80,
                p95_us: 80,
                p99_us: 80,
                sum_us: 800,
            },
        );

        let mut run = baseline.clone();
        run.spans.insert(
            END_TO_END_SPAN.to_string(),
            SpanMetric {
                count: 10,
                p50_us: 130,
                p95_us: 130,
                p99_us: 130,
                sum_us: 1_300,
            },
        );
        run.spans.insert(
            DF_COLLECT_SPAN.to_string(),
            SpanMetric {
                count: 10,
                p50_us: 40,
                p95_us: 40,
                p99_us: 40,
                sum_us: 400,
            },
        );

        let mut failures = Vec::new();
        compare_artifacts(&mut failures, &run, &baseline, BenchTier::Tier0);

        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("bench.query.end_to_end")),
            "{failures:?}"
        );
    }

    #[test]
    fn object_store_comparison_uses_rate_per_end_to_end() {
        let mut failures = Vec::new();
        compare_rate(
            &mut failures,
            "object_store_counts.get_range",
            2_000,
            1_000,
            1_000,
            500,
            BenchTier::Tier0,
        );

        assert!(failures.is_empty(), "{failures:?}");
    }
}
