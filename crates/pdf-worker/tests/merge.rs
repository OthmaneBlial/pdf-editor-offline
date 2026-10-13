use pdf_worker::{Request, document::execute_document};
#[test]
fn merge_rejects_paths_invalid_frames_and_missing_inputs_before_parsing() {
    for control in [
        serde_json::json!({"operation":"merge","lengths":[1,1],"path":"private.pdf"}),
        serde_json::json!({"operation":"merge","lengths":[]}),
        serde_json::json!({"operation":"merge","lengths":[1]}),
        serde_json::json!({"operation":"merge","lengths":[0,2]}),
        serde_json::json!({"operation":"merge","lengths":[1,usize::MAX]}),
        serde_json::json!({"operation":"merge","lengths":[1,1]}),
    ] {
        assert!(
            execute_document(Request {
                control,
                payload: vec![1, 2, 3]
            })
            .is_err()
        );
    }
}
