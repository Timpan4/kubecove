// EOF must release the upload sender without cancelling response delivery.
// Stale IDs must not affect any other connection, and repeated EOF is harmless.
#[tokio::test]
async fn upload_eof_preserves_connection_until_response_finishes() {
    let registry = super::TunnelRegistry::default();
    let id = "123e4567-e89b-12d3-a456-426614174000";
    let (incoming, mut receiver) = tokio::sync::mpsc::channel(1);
    let task = tokio::spawn(std::future::pending());
    registry.state.lock().unwrap().connections.insert(
        id.to_string(),
        super::TunnelConnection {
            forward_id: id.to_string(),
            incoming: Some(incoming),
            task,
        },
    );
    assert!(!registry.finish_upload("123e4567-e89b-12d3-a456-426614174001"));
    assert!(registry.finish_upload(id));
    assert!(registry.finish_upload(id));
    assert_eq!(receiver.recv().await, None);
    assert!(!registry.state.lock().unwrap().connections[id]
        .task
        .is_finished());
    assert!(registry.close_connection(id));
    assert!(registry.state.lock().unwrap().connections.is_empty());
}
