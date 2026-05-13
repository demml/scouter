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
    pub spans: BTreeMap<String, SpanMetric>,
    pub object_store_counts: ObjectStoreCountSnapshot,
    #[serde(default)]
    pub refresh_on_request_path_total: u64,
}

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
        compare_count(
            &mut failures,
            "object_store_counts.get_range",
            run.object_store_counts.get_range,
            baseline.object_store_counts.get_range,
            run_tier,
        );
        compare_count(
            &mut failures,
            "object_store_counts.head",
            run.object_store_counts.head,
            baseline.object_store_counts.head,
            run_tier,
        );
        compare_count(
            &mut failures,
            "object_store_counts.list",
            run.object_store_counts.list,
            baseline.object_store_counts.list,
            run_tier,
        );
        compare_span(&mut failures, "df.collect", &run, &baseline, run_tier);
        compare_span(
            &mut failures,
            "delta.snapshot.refresh",
            &run,
            &baseline,
            run_tier,
        );
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

fn compare_requested_tier(tier: BenchTier) -> Result<(), String> {
    let dir = run_metrics_dir();
    if !dir.exists() {
        return Ok(());
    }

    let mut notes = Vec::new();
    let mut compared = 0usize;
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
        match compare_artifact(&path, tier) {
            Ok(artifact_notes) => notes.extend(artifact_notes),
            Err(err) if tier != BenchTier::Tier0 => notes.push(format!(
                "advisory tier ignored comparator error for {}: {err}",
                path.display()
            )),
            Err(err) => return Err(err),
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
}
