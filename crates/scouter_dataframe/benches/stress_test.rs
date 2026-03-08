mod utils;

use chrono::Utc;
use scouter_dataframe::parquet::tracing::service::TraceSpanService;
use scouter_settings::ObjectStorageSettings;
use scouter_types::TraceId;
use std::time::Instant;

const TOTAL_SPANS: usize = 1_000_000;
const HOURS: usize = 24;
const SPANS_PER_HOUR: usize = TOTAL_SPANS / HOURS; // ~41,667
const TRACES_PER_HOUR: usize = SPANS_PER_HOUR / 5; // ~8,333 (5 spans/trace)
const QUERY_ITERS: usize = 500;
const TARGET_ENTITY_UID: &str = "stress-entity-abc123";
const ENTITY_TRACES: usize = 50;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    // Clean up previous run's storage so each run starts fresh.
    std::fs::remove_dir_all("./scouter_storage").ok();

    println!("=== Delta Lake At-Scale Stress Benchmark ===\n");
    println!(
        "Config: {} total spans, {} hourly batches (~{} spans/batch), {} query iters\n",
        TOTAL_SPANS, HOURS, SPANS_PER_HOUR, QUERY_ITERS
    );

    // compaction_interval_hours=999 disables auto-compaction; flush every 5s (unused for direct writes).
    let storage_settings = ObjectStorageSettings::default();
    let service = TraceSpanService::new(&storage_settings, 999, Some(5)).await?;

    // ── Phase 1: Seed ──────────────────────────────────────────────────────
    println!(
        "Phase 1: Seeding {} spans via direct writes ({} hourly batches)...",
        TOTAL_SPANS, HOURS
    );
    let seed_start = Instant::now();

    // Collect one trace_id per hour for later query benchmarks.
    let mut all_ids: Vec<(usize, Vec<Vec<u8>>)> = Vec::with_capacity(HOURS);

    for hour in 0..HOURS {
        let minutes_offset = (hour as i64) * 60;
        let mut hour_spans = Vec::with_capacity(SPANS_PER_HOUR);
        let mut hour_ids: Vec<Vec<u8>> = Vec::new();

        for _ in 0..TRACES_PER_HOUR {
            let (_r, spans, _t) = scouter_mocks::generate_trace_with_spans(5, minutes_offset);
            if hour_ids.len() < 500 {
                let id_bytes = TraceId::hex_to_bytes(&spans[0].trace_id.to_hex())?;
                hour_ids.push(id_bytes);
            }
            hour_spans.extend(spans);
        }

        service.write_spans_direct(hour_spans).await?;
        all_ids.push((hour, hour_ids));

        if hour % 6 == 5 {
            println!(
                "  hour {}/{} seeded ({:.1}s elapsed)",
                hour + 1,
                HOURS,
                seed_start.elapsed().as_secs_f64()
            );
        }
    }

    // Seed entity spans at hour 0 (now - 0 minutes offset).
    {
        let entity_spans = utils::create_entity_trace_batch(ENTITY_TRACES, TARGET_ENTITY_UID);
        service.write_spans_direct(entity_spans).await?;
    }

    println!(
        "  Seeded {} spans in {:.2}s ({:.0} spans/sec)\n",
        TOTAL_SPANS,
        seed_start.elapsed().as_secs_f64(),
        TOTAL_SPANS as f64 / seed_start.elapsed().as_secs_f64()
    );

    // ── Phase 2: Z-ORDER compaction ────────────────────────────────────────
    println!("Phase 2: Z-ORDER compaction on {} spans...", TOTAL_SPANS);
    let opt_start = Instant::now();
    service.optimize().await?;
    println!("  Compaction: {:.2}s\n", opt_start.elapsed().as_secs_f64());

    // ── Phase 3: Query benchmarks ──────────────────────────────────────────
    println!(
        "Phase 3: Query benchmarks ({} iterations each)...",
        QUERY_ITERS
    );

    // 3a. trace_id lookup — no time bounds (full scan baseline).
    {
        let mut timings = Vec::with_capacity(QUERY_ITERS);
        for i in 0..QUERY_ITERS {
            let (_, ids) = &all_ids[i % HOURS];
            let id = &ids[i % ids.len()];
            let t = Instant::now();
            let _ = service
                .query_service
                .query_spans(Some(id), None, None, None, None, None)
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "query_spans (by trace_id, no time bounds)",
            &utils::compute_percentiles(timings),
        );
    }

    // 3b. trace_id + 1h time bound — validates Parquet row-group pruning.
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
                .query_spans(Some(id), None, Some(&start_t), Some(&end_t), None, None)
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "query_spans (by trace_id, 1h time bound)",
            &utils::compute_percentiles(timings),
        );
    }

    // 3c. entity_uid + 1h time bound — validates entity_id column pruning.
    {
        let now = Utc::now();
        let start_t = now - chrono::Duration::hours(1);
        let end_t = now + chrono::Duration::minutes(5);
        let mut timings = Vec::with_capacity(QUERY_ITERS);
        for _ in 0..QUERY_ITERS {
            let t = Instant::now();
            let _ = service
                .query_service
                .query_spans(
                    None,
                    None,
                    Some(&start_t),
                    Some(&end_t),
                    None,
                    Some(TARGET_ENTITY_UID),
                )
                .await?;
            timings.push(t.elapsed());
        }
        utils::print_percentiles(
            "query_spans (by entity_uid, 1h time bound)",
            &utils::compute_percentiles(timings),
        );
    }

    println!("\n=== Stress Benchmark Complete ===");
    println!("Pruning proof: compare p50 of '1h time bound' vs 'no time bounds'.");
    println!(
        "If the bounded query is >20% faster, Parquet file-level min/max pruning is working.\n"
    );
    Ok(())
}
