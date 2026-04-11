use chrono::Utc;
use scouter_dataframe::{EvalScenarioRecord, EvalScenarioService};
use scouter_settings::ObjectStorageSettings;

fn local_settings(dir: &std::path::Path) -> ObjectStorageSettings {
    ObjectStorageSettings {
        storage_uri: dir.to_string_lossy().to_string(),
        ..ObjectStorageSettings::default()
    }
}

fn make_record(collection_id: &str, scenario_id: &str) -> EvalScenarioRecord {
    EvalScenarioRecord {
        collection_id: collection_id.to_string(),
        scenario_id: scenario_id.to_string(),
        scenario_json: r#"{"id":"s1","input":{"prompt":"hi"},"expected_output":{"text":"hello"}}"#
            .to_string(),
        created_at: Utc::now(),
    }
}

#[tokio::test]
async fn test_write_and_query_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let settings = local_settings(dir.path());
    let service = EvalScenarioService::new(&settings).await.unwrap();

    let records = vec![
        make_record("col-1", "s1"),
        make_record("col-1", "s2"),
        make_record("col-1", "s3"),
    ];

    service.write_scenarios(records).await.unwrap();

    let results = service.get_scenarios("col-1").await.unwrap();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r.collection_id == "col-1"));
}

#[tokio::test]
async fn test_two_collections_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let settings = local_settings(dir.path());
    let service = EvalScenarioService::new(&settings).await.unwrap();

    service
        .write_scenarios(vec![make_record("col-a", "s1"), make_record("col-a", "s2")])
        .await
        .unwrap();

    service
        .write_scenarios(vec![make_record("col-b", "s3")])
        .await
        .unwrap();

    let col_a = service.get_scenarios("col-a").await.unwrap();
    let col_b = service.get_scenarios("col-b").await.unwrap();

    assert_eq!(col_a.len(), 2);
    assert_eq!(col_b.len(), 1);
    assert!(col_a.iter().all(|r| r.collection_id == "col-a"));
    assert!(col_b.iter().all(|r| r.collection_id == "col-b"));
}

#[tokio::test]
async fn test_empty_table_returns_empty_vec() {
    let dir = tempfile::tempdir().unwrap();
    let settings = local_settings(dir.path());
    let service = EvalScenarioService::new(&settings).await.unwrap();

    let results = service
        .get_scenarios("nonexistent-collection")
        .await
        .unwrap();

    assert!(results.is_empty());
}
