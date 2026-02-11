mod utils;

use scouter_dataframe::parquet::tracing::service::TraceSpanService;
use scouter_settings::ObjectStorageSettings;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🚀 Starting TraceSpanService Stress Test");
    println!("=========================================\n");

    // Test configuration
    let storage_settings = ObjectStorageSettings::default();
    let service = Arc::new(TraceSpanService::new(&storage_settings, 24, Some(5)).await?);

    // Target: 20M spans/day = ~231 spans/second
    let target_spans_per_sec = 231;
    let test_duration_secs = 60;
    let concurrent_writers = 8;

    println!("Configuration:");
    println!("  - Target: {} spans/second", target_spans_per_sec);
    println!("  - Test Duration: {} seconds", test_duration_secs);
    println!("  - Concurrent Writers: {}", concurrent_writers);
    println!("  - Buffer Size: 10,000 spans");
    println!("  - Flush Interval: 5 seconds\n");

    let semaphore = Arc::new(Semaphore::new(concurrent_writers));
    let start_time = Instant::now();
    let mut total_spans_written = 0;
    let mut tasks = vec![];

    println!("📊 Writing spans...");

    while start_time.elapsed().as_secs() < test_duration_secs {
        let permit = semaphore.clone().acquire_owned().await?;
        let service_clone = service.clone();

        let task = tokio::spawn(async move {
            let spans = utils::create_simple_trace();
            let span_count = spans.len();

            if let Err(e) = service_clone.write_spans(spans).await {
                eprintln!("Write error: {}", e);
                return 0;
            }

            drop(permit);
            span_count
        });

        tasks.push(task);

        // Rate limiting to hit target throughput
        let target_interval = Duration::from_micros(1_000_000 / target_spans_per_sec as u64 * 3);
        tokio::time::sleep(target_interval).await;
    }

    println!("⏳ Waiting for all writes to complete...");

    for task in tasks {
        if let Ok(count) = task.await {
            total_spans_written += count;
        }
    }

    let elapsed = start_time.elapsed();
    let actual_rate = total_spans_written as f64 / elapsed.as_secs_f64();

    println!("\n✅ Write Phase Complete");
    println!("  - Total Spans Written: {}", total_spans_written);
    println!("  - Elapsed Time: {:.2}s", elapsed.as_secs_f64());
    println!("  - Actual Rate: {:.2} spans/second", actual_rate);
    println!("  - Target Rate: {} spans/second", target_spans_per_sec);
    println!(
        "  - Efficiency: {:.1}%",
        (actual_rate / target_spans_per_sec as f64) * 100.0
    );

    println!("\n🔍 Running Query Performance Test...");

    let query_start = Instant::now();
    let results = service
        .query_service
        .get_trace_spans(None, None, None, None, Some(1000))
        .await?;

    let total_queried: usize = results.iter().map(|batch| batch.len()).sum();
    let query_duration = query_start.elapsed();

    println!(
        "  - Queried {} spans in {:.2}ms",
        total_queried,
        query_duration.as_millis()
    );
    println!(
        "  - Query Rate: {:.2} spans/ms",
        total_queried as f64 / query_duration.as_millis() as f64
    );

    println!("\n🔧 Running Optimization...");
    let optimize_start = Instant::now();
    service.optimize().await?;
    println!(
        "  - Optimization completed in {:.2}s",
        optimize_start.elapsed().as_secs_f64()
    );

    println!("\n🛑 Shutting down service...");
    // Drop the Arc to allow shutdown. Since all tasks are complete,
    // this is the last reference and will trigger cleanup.
    drop(service);

    println!("\n✨ Stress Test Complete!\n");

    Ok(())
}
