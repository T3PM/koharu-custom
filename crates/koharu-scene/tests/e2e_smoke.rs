use koharu_scene::*;

#[tokio::test]
async fn smoke_test_scene_integration() {
    let session = Session::memory().await.unwrap();
    let snapshot = session.snapshot();
    assert_eq!(snapshot.revision(), Revision::ZERO);
}
