//! Adversarial Stress Test Suite for Milestone 1
//! Empirical Challenger 2 verification for:
//! - M12: Entity Removal Contract (`remove_entity` vs `remove_entity_if_present`)
//! - H7: Speaker Origin Provenance (`Origin::User` vs `Origin::Generated`)
//! - L5: Dead Wrapper Pruning & Direct BalloonClassification Assignment

use koharu_scene::*;

fn sample_page(name: &str) -> PageDraft {
    PageDraft::new(name, 1200.0, 1800.0)
}

fn sample_source(text: &str) -> SourceText {
    SourceText {
        text: Authored::user(text.to_owned()),
        language: Some(LanguageTag::new("ja").unwrap()),
    }
}


#[tokio::test]
async fn test_remove_entity_strictly_returns_entity_not_found_on_missing_entities() {
    let session = Session::memory().await.unwrap();

    for policy in [
        RemovePolicy::Cascade,
        RemovePolicy::RejectNonEmpty,
        RemovePolicy::PromoteChildren,
    ] {
        let missing_id = EntityId::new();
        let result = session
            .snapshot()
            .patch(|edit| edit.remove_entity(missing_id, policy));

        match result {
            Err(Error::EntityNotFound(id)) => {
                assert_eq!(id, missing_id, "Returned EntityNotFound must match the requested id");
            }
            Ok(_) => panic!("remove_entity with policy {policy:?} on missing entity unexpectedly succeeded"),
            Err(other) => panic!("expected Error::EntityNotFound, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn test_remove_entity_strictly_fails_on_cascaded_and_double_deleted_entities() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;
    let mut content_id = None;
    let mut layer_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(sample_page("p1"), At::End)?;
            let c = edit.add_text_content(p, At::End)?;
            edit.set(c, &sample_source("Hierarchical test"))?;
            let l = edit.add_text_layer(
                p,
                At::End,
                c,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            page_id = Some(p);
            content_id = Some(c);
            layer_id = Some(l);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();

    let page = page_id.unwrap();
    let content = content_id.unwrap();
    let layer = layer_id.unwrap();

    let del_patch = session
        .snapshot()
        .patch(|edit| {
            edit.remove_entity(page, RemovePolicy::Cascade)?;
            Ok(())
        })
        .unwrap();
    session.commit(del_patch).await.unwrap();

    let snap = session.snapshot();
    for entity in [page, content, layer] {
        let res = snap.patch(|edit| edit.remove_entity(entity, RemovePolicy::Cascade));
        match res {
            Err(Error::EntityNotFound(id)) => assert_eq!(id, entity),
            other => panic!("expected EntityNotFound({entity:?}), got {other:?}"),
        }
    }
}

#[tokio::test]
async fn test_remove_entity_if_present_missing_vs_existing_idempotency_and_policy_preservation() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;
    let mut content_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(sample_page("p1"), At::End)?;
            let c = edit.add_text_content(p, At::End)?;
            edit.set(c, &sample_source("Idempotency test"))?;
            page_id = Some(p);
            content_id = Some(c);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();

    let page = page_id.unwrap();
    let content = content_id.unwrap();
    let missing_id = EntityId::new();

    let patch = session
        .snapshot()
        .patch(|edit| {
            let missing_res = edit.remove_entity_if_present(missing_id, RemovePolicy::Cascade)?;
            assert!(!missing_res, "missing entity removal must return false");

            let existing_res = edit.remove_entity_if_present(content, RemovePolicy::Cascade)?;
            assert!(existing_res, "existing entity removal must return true");

            let second_res = edit.remove_entity_if_present(content, RemovePolicy::Cascade)?;
            assert!(!second_res, "subsequent removal of same entity must return false");

            Ok(())
        })
        .unwrap();
    session.commit(patch).await.unwrap();

    let add_child = session
        .snapshot()
        .patch(|edit| {
            let c2 = edit.add_text_content(page, At::End)?;
            edit.set(c2, &sample_source("Child entity"))?;
            Ok(())
        })
        .unwrap();
    session.commit(add_child).await.unwrap();

    let policy_res = session.snapshot().patch(|edit| {
        edit.remove_entity_if_present(page, RemovePolicy::RejectNonEmpty)?;
        Ok(())
    });
    match policy_res {
        Err(Error::NonEmptyEntity(id)) => assert_eq!(id, page),
        other => panic!("expected Error::NonEmptyEntity, got {other:?}"),
    }
}


#[tokio::test]
async fn test_speaker_origin_provenance_user_vs_generated_and_undo_redo() {
    let mut session = Session::memory().await.unwrap();
    let entry_1 = GlossaryEntryId::new();
    let entry_2 = GlossaryEntryId::new();
    let gen_producer = ProducerId::new("dev.koharu.pipeline.speaker-classifier").unwrap();
    let generation = Generation::new(gen_producer);

    let mut layer_id = None;
    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(sample_page("p1"), At::End)?;
            let c = edit.add_text_content(p, At::End)?;
            let l = edit.add_text_layer(
                p,
                At::End,
                c,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            layer_id = Some(l);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();
    let layer = layer_id.unwrap();

    let gen_patch = session
        .snapshot()
        .patch(|edit| {
            edit.set_speaker(layer, Some(entry_1), Origin::Generated(generation.clone()))
        })
        .unwrap();
    let snap_gen = session.commit(gen_patch).await.unwrap().snapshot;
    let s1 = snap_gen.text_layer(layer).unwrap().speaker().unwrap().unwrap();
    assert_eq!(s1.speaker, Some(entry_1));
    assert_eq!(s1.origin, Origin::Generated(generation.clone()));

    let user_patch = snap_gen
        .patch(|edit| {
            edit.set_speaker(layer, Some(entry_2), Origin::User)
        })
        .unwrap();
    let commit_user = session.commit(user_patch).await.unwrap();
    let s2 = commit_user.snapshot.text_layer(layer).unwrap().speaker().unwrap().unwrap();
    assert_eq!(s2.speaker, Some(entry_2));
    assert_eq!(s2.origin, Origin::User);

    let undo_commit = session.undo(commit_user.revision).await.unwrap();
    let s_undo = undo_commit.snapshot.text_layer(layer).unwrap().speaker().unwrap().unwrap();
    assert_eq!(s_undo.speaker, Some(entry_1));
    assert_eq!(s_undo.origin, Origin::Generated(generation.clone()));

    let redo_commit = session.undo(undo_commit.revision).await.unwrap();
    let s_redo = redo_commit.snapshot.text_layer(layer).unwrap().speaker().unwrap().unwrap();
    assert_eq!(s_redo.speaker, Some(entry_2));
    assert_eq!(s_redo.origin, Origin::User);
}


#[tokio::test]
async fn test_balloon_classification_direct_edit_set_all_shapes() {
    let mut session = Session::memory().await.unwrap();
    let mut layer_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(sample_page("p1"), At::End)?;
            let c = edit.add_text_content(p, At::End)?;
            let l = edit.add_text_layer(
                p,
                At::End,
                c,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            layer_id = Some(l);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();
    let layer = layer_id.unwrap();

    let shapes = [
        BalloonShape::NormalSpeech,
        BalloonShape::Whisper,
        BalloonShape::Shout,
        BalloonShape::Thought,
        BalloonShape::Narration,
    ];

    for shape in shapes {
        let patch = session
            .snapshot()
            .patch(|edit| {
                edit.set(
                    layer,
                    &BalloonClassification {
                        origin: Origin::User,
                        shape,
                        confidence: 0.95,
                    },
                )
            })
            .unwrap();
        let snap = session.commit(patch).await.unwrap().snapshot;
        let class = snap.text_layer(layer).unwrap().balloon_classification().unwrap().unwrap();
        assert_eq!(class.shape, shape);
        assert_eq!(class.origin, Origin::User);
        assert!((class.confidence - 0.95).abs() < 1e-4);
    }
}
