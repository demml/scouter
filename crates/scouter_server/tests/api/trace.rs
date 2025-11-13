use crate::common::TestHelper;

#[tokio::test]
async fn test_tracing() {
    let helper = TestHelper::new(false, false).await.unwrap();
    helper.generate_trace_data().await.unwrap();
}
