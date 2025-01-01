pub async fn get_observability_metrics(
    State(data): State<Arc<AppState>>,
    params: Query<ObservabilityMetricRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let query_result = &data.db.get_binned_observability_metrics(&params).await;

    match query_result {
        Ok(result) => {
            let json_response = serde_json::json!({
                "status": "success",
                "data": result
            });
            Ok(Json(json_response))
        }
        Err(e) => {
            error!("Failed to query observability_metrics: {:?}", e);
            let json_response = json!({
                "status": "error",
                "message": format!("{:?}", e)
            });
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json_response)))
        }
    }
}