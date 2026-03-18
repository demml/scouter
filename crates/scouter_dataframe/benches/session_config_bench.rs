//! Focused benchmark for comparing DataFusion session config tuning.
//!
//! Seeds 100K spans, then measures query latency across four access patterns.
//!
//! ```bash
//! # Local FS (default SCOUTER_STORAGE_URI=./scouter_storage):
//! cargo bench -p scouter-dataframe --bench session_config_bench
//!
//! # GCS:
//! SCOUTER_STORAGE_URI=gs://your-bucket cargo bench -p scouter-dataframe --bench session_config_bench
//! ```

mod utils;

use chrono::Utc;
use scouter_dataframe::parquet::tracing::service::TraceSpanService;
use scouter_settings::ObjectStorageSettings;
use scouter_types::TraceId;
use std::time::Instant;

const TOTAL_SPANS: usize = 100_000;
const HOURS: usize = 12;
const SPANS_PER_HOUR: usize = TOTAL_SPANS / HOURS;
const TRACES_PER_HOUR: usize = SPANS_PER_HOUR / 5;
const QUERY_ITERS: usize = 200;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    // Clean up previous run's storage so each run starts fresh.
    let storage_settings = ObjectStorageSettings::default();

    println!("=== Session Config A/B Benchmark ===\n");
    println!("Backend: {}", storage_settings.storage_uri);
    println!(
        "Config: {} spans, {} hourly batches, {} query iters\n",
        TOTAL_SPANS, HOURS, QUERY_ITERS
    );

    let service = TraceSpanService::new(&storage_settings, 999, Some(1), None, 10).await?;

    // ── Seed ─────────────────────────────────────────────────────────────
    println!("Seeding {} spans...", TOTAL_SPANS);
    let seed_start = Instant::now();
    let mut all_ids: Vec<(usize, Vec<Vec<u8>>)> = Vec::with_capacity(HOURS);

    for hour in 0..HOURS {
        let minutes_offset = (hour as i64) * 60;
        let mut hour_spans = Vec::with_capacity(SPANS_PER_HOUR);
        let mut hour_ids: Vec<Vec<u8>> = Vec::new();

        for _ in 0..TRACES_PER_HOUR {
            let (_r, spans, _t) = scouter_mocks::generate_trace_with_spans(5, minutes_offset);
            if hour_ids.len() < 100 {
                let id_bytes = TraceId::hex_to_bytes(&spans[0].trace_id.to_hex())?;
                hour_ids.push(id_bytes);
            }
            hour_spans.extend(spans);
        }

        service.write_spans_direct(hour_spans).await?;
        all_ids.push((hour, hour_ids));

        if hour % 3 == 2 {
            println!(
                "  hour {}/{} seeded ({:.1}s elapsed)",
                hour + 1,
                HOURS,
                seed_start.elapsed().as_secs_f64()
            );
        }
    }

    println!("  Seeded in {:.2}s\n", seed_start.elapsed().as_secs_f64());

    // ── Compact ──────────────────────────────────────────────────────────
    println!("Compacting...");
    let opt_start = Instant::now();
    service.optimize().await?;
    println!("  Compacted in {:.2}s\n", opt_start.elapsed().as_secs_f64());

    // ── Warmup (5 queries, discarded) ────────────────────────────────────
    println!("Warming up...");
    for i in 0..5 {
        let (_, ids) = &all_ids[i % HOURS];
        let id = &ids[0];
        let _ = service
            .query_service
            .query_spans(Some(id), None, None, None, None)
            .await?;
    }

    println!("Running queries...\n");

    // ── Bench 1: trace_id, no time bounds ────────────────────────────────
    {
        let mut timings = Vec::with_capacity(QUERY_ITERS);
        for i in 0..QUERY_ITERS {
            let (_, ids) = &all_ids[i % HOURS];
            let id = &ids[i % ids.len()];
            let t = Instant::now();
            let _ = service
                .query_service
                .query_spans(Some(id), None, None, None, None)
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "trace_id lookup (no time bounds)",
            &utils::compute_percentiles(timings),
        );
    }

    // ── Bench 2: trace_id + 1h time bound ────────────────────────────────
    {
        let now = Utc::now();
        let mut timings = Vec::with_capacity(QUERY_ITERS);
        for i in 0..QUERY_ITERS {
            let hour = i % HOURS;
            let (_, ids) = &all_ids[hour];
            let id = &ids[i % ids.len()];
            let start_t = now - chrono::Duration::hours((hour as i64) + 1);
            let end_t = now - chrono::Duration::hours(hour as i64);
            let t = Instant::now();
            let _ = service
                .query_service
                .query_spans(Some(id), None, Some(&start_t), Some(&end_t), None)
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "trace_id lookup (1h time bound)",
            &utils::compute_percentiles(timings),
        );
    }

    // ── Bench 3: time-window scan (last 24h, limit 1000) ────────────────
    {
        let now = Utc::now();
        let start_t = now - chrono::Duration::hours(24);
        let end_t = now + chrono::Duration::hours(1);
        let mut timings = Vec::with_capacity(QUERY_ITERS);
        for _ in 0..QUERY_ITERS {
            let t = Instant::now();
            let _ = service
                .query_service
                .query_spans(None, None, Some(&start_t), Some(&end_t), Some(1000))
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "time-window scan (24h, limit 1000)",
            &utils::compute_percentiles(timings),
        );
    }

    // ── Bench 4: repeated queries (measures metadata caching benefit) ────
    {
        let (_, ids) = &all_ids[0];
        let id = &ids[0];
        let mut timings = Vec::with_capacity(QUERY_ITERS);
        for _ in 0..QUERY_ITERS {
            let t = Instant::now();
            let _ = service
                .query_service
                .query_spans(Some(id), None, None, None, None)
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "same trace_id repeated (cache effect)",
            &utils::compute_percentiles(timings),
        );
    }

    service.shutdown().await?;

    println!("\n=== Session Config Benchmark Complete ===");
    Ok(())
}
