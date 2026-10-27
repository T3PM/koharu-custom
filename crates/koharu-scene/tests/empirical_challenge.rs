//! Empirical challenge and stress tests for C1 (Schema Revision) and C2 (Patch State Accumulation).
//! Written by Milestone 1 Challenger 1 (teamwork_preview_challenger).

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

// =========================================================================
// C1: Schema 1 Binary Decode Robustness & Payload Corruption Stress
// =========================================================================

const VALID_SCHEMA_1_ENTRY_BYTES: &[u8] = &[
    // Revision header: 1
    0x01,
    // id: GlossaryEntryId (revision header: 1, followed by 16-byte Uuid 00000000-0000-0000-0000-000000000001)
    0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
    // source: "山田" (len = 6, UTF-8 bytes)
    0x06, 0xe5, 0xb1, 0xb1, 0xe7, 0x94, 0xb0,
    // translation: Some("Yamada") (tag = 1, len = 6, ASCII bytes)
    0x01, 0x06, 0x59, 0x61, 0x6d, 0x61, 0x64, 0x61,
    // kind: GlossaryKind::Person (revision: 1, variant: 0)
    0x01, 0x00,
    // enabled: true
    0x01,
    // confidence: Some(1.0f32) (tag = 1, 4 bytes LE)
    0x01, 0x00, 0x00, 0x80, 0x3f,
    // occurrence_count: 5
    0x05,
    // examples: vec![] (len = 0)
    0x00,
    // source_origin: GlossaryValueOrigin::User (revision: 1, variant: 2)
    0x01, 0x02,
    // translation_origin: Some(GlossaryValueOrigin::User) (tag = 1, revision: 1, variant: 2)
    0x01, 0x01, 0x02,
    // present_in_last_scan: true
    0x01,
];

#[test]
fn test_c1_schema_1_valid_decode_all_fields() {
    let entry: GlossaryEntry = revision::from_slice(VALID_SCHEMA_1_ENTRY_BYTES)
        .expect("Valid schema-1 bytes must decode cleanly into schema-2 GlossaryEntry");

    assert_eq!(entry.id.as_uuid(), uuid::Uuid::from_u128(1));
    assert_eq!(entry.source, "山田");
    assert_eq!(entry.translation.as_deref(), Some("Yamada"));
    assert_eq!(entry.kind, GlossaryKind::Person);
    assert!(entry.enabled);
    assert_eq!(entry.confidence, Some(1.0));
    assert_eq!(entry.occurrence_count, 5);
    assert!(entry.examples.is_empty());
    assert_eq!(entry.source_origin, GlossaryValueOrigin::User);
    assert_eq!(entry.translation_origin, Some(GlossaryValueOrigin::User));
    assert!(entry.present_in_last_scan);
    assert_eq!(entry.character, None);
}

#[test]
fn test_c1_truncated_payloads_never_panic() {
    for len in 0..VALID_SCHEMA_1_ENTRY_BYTES.len() {
        let truncated = &VALID_SCHEMA_1_ENTRY_BYTES[..len];
        let result = revision::from_slice::<GlossaryEntry>(truncated);
        assert!(
            result.is_err(),
            "Truncating schema-1 payload at byte {len} must return Err safely, never panic"
        );
    }
}

#[test]
fn test_c1_corrupted_revision_headers() {
    let mut payload = VALID_SCHEMA_1_ENTRY_BYTES.to_vec();

    payload[0] = 0x00;
    assert!(revision::from_slice::<GlossaryEntry>(&payload).is_err());

    payload[0] = 0x03;
    assert!(revision::from_slice::<GlossaryEntry>(&payload).is_err());

    payload[0] = 0xFF;
    assert!(revision::from_slice::<GlossaryEntry>(&payload).is_err());
}

#[test]
fn test_c1_corrupted_utf8_in_source() {
    let mut payload = VALID_SCHEMA_1_ENTRY_BYTES.to_vec();
    payload[18] = 0xFF;
    payload[19] = 0xFF;
    assert!(
        revision::from_slice::<GlossaryEntry>(&payload).is_err(),
        "Invalid UTF-8 in source field must fail decode safely without panic"
    );
}

#[test]
fn test_c1_corrupted_enum_variants() {
    let mut payload = VALID_SCHEMA_1_ENTRY_BYTES.to_vec();
    payload[32] = 0xFE; // Invalid enum variant
    assert!(
        revision::from_slice::<GlossaryEntry>(&payload).is_err(),
        "Invalid enum tag for GlossaryKind must fail decode safely"
    );

    let mut payload2 = VALID_SCHEMA_1_ENTRY_BYTES.to_vec();
    payload2[43] = 0xFE;
    assert!(
        revision::from_slice::<GlossaryEntry>(&payload2).is_err(),
        "Invalid enum tag for GlossaryValueOrigin must fail decode safely"
    );
}

#[test]
fn test_c1_corrupted_boolean_bytes() {
    let mut payload = VALID_SCHEMA_1_ENTRY_BYTES.to_vec();
    payload[33] = 0x02; // Invalid boolean byte
    assert!(
        revision::from_slice::<GlossaryEntry>(&payload).is_err(),
        "Invalid bool byte must fail decode safely"
    );
}

#[tokio::test]
async fn test_c1_semantic_validation_boundaries_via_session() {
    let session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();

    let valid_entry = GlossaryEntry {
        id: entry_id,
        source: "Valid Source".to_owned(),
        translation: Some("Valid Translation".to_owned()),
        kind: GlossaryKind::Person,
        enabled: true,
        confidence: Some(0.95),
        occurrence_count: 1,
        examples: vec!["Example 1".to_owned()],
        source_origin: GlossaryValueOrigin::User,
        translation_origin: Some(GlossaryValueOrigin::User),
        present_in_last_scan: true,
        character: None,
    };
    let glossary = Glossary {
        enabled: true,
        source_language: None,
        target_language: None,
        source_fingerprint: None,
        entries: vec![valid_entry],
    };
    assert!(session.snapshot().patch(|edit| edit.set_project(&glossary)).is_ok());

    // Confidence out of range (> 1.0)
    let mut invalid_conf = glossary.clone();
    invalid_conf.entries[0].confidence = Some(1.05);
    assert!(session.snapshot().patch(|edit| edit.set_project(&invalid_conf)).is_err());

    // Confidence NaN
    let mut nan_conf = glossary.clone();
    nan_conf.entries[0].confidence = Some(f32::NAN);
    assert!(session.snapshot().patch(|edit| edit.set_project(&nan_conf)).is_err());

    // Source contains control characters
    let mut ctrl_source = glossary.clone();
    ctrl_source.entries[0].source = "Invalid\nSource".to_owned();
    assert!(session.snapshot().patch(|edit| edit.set_project(&ctrl_source)).is_err());

    // Empty source
    let mut empty_source = glossary.clone();
    empty_source.entries[0].source = "   ".to_owned();
    assert!(session.snapshot().patch(|edit| edit.set_project(&empty_source)).is_err());

    // Character profile with mismatched ID
    let mut mismatched_char = glossary.clone();
    mismatched_char.entries[0].character = Some(CharacterProfile {
        id: GlossaryEntryId::new(), // Mismatch
        gender: CharacterGender::Unspecified,
        first_person: None,
        second_person: None,
        tone: None,
        custom_instructions: None,
    });
    assert!(session.snapshot().patch(|edit| edit.set_project(&mismatched_char)).is_err());
}

// =========================================================================
// C2: Patch State Accumulation and Pruning Stress Tests
// =========================================================================

#[tokio::test]
async fn test_c2_empty_patch_handling() {
    let mut session = Session::memory().await.unwrap();
    let initial_revision = session.snapshot().revision();

    // Create a patch with no modifications
    let empty_patch = session.snapshot().patch(|_edit| Ok(())).unwrap();
    assert!(empty_patch.is_empty());

    let (pruned, diagnostics) = empty_patch.prune_missing_with_diagnostics(&session.snapshot());
    assert!(diagnostics.is_empty());
    assert!(pruned.is_empty());

    // Commit empty patch
    let commit = session.commit(pruned).await.unwrap();
    assert_eq!(commit.revision, initial_revision);
    assert_eq!(session.snapshot().revision(), initial_revision);
}

#[tokio::test]
async fn test_c2_complex_chained_operations_accumulate_without_dropping() {
    let mut session = Session::memory().await.unwrap();
    let base_snap = session.snapshot();
    let entry_id = GlossaryEntryId::new();

    let mut page_id = None;
    let mut layer_1_id = None;
    let mut layer_2_id = None;
    let mut bubble_1_id = None;
    let mut bubble_2_id = None;

    // Single patch creating a rich hierarchy and multiple relations
    let patch = base_snap
        .patch(|edit| {
            // 1. Add page
            let page = edit.add_page(sample_page("p_main"), At::End)?;
            page_id = Some(page);

            // 2. Add content 1 & layer 1
            let content_1 = edit.add_text_content(page, At::End)?;
            edit.set(content_1, &sample_source("セリフ１"))?;
            let layer_1 = edit.add_text_layer(
                page,
                At::End,
                content_1,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            edit.set_speaker(layer_1, Some(entry_id), Origin::User)?;
            layer_1_id = Some(layer_1);

            // 3. Add content 2 & layer 2
            let content_2 = edit.add_text_content(page, At::End)?;
            edit.set(content_2, &sample_source("セリフ２"))?;
            let layer_2 = edit.add_text_layer(
                page,
                At::End,
                content_2,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            layer_2_id = Some(layer_2);

            // 4. Add bubble regions 1 & 2
            let bubble_1 = edit.add_analysis_region::<BubbleRegion>(
                page,
                At::End,
                &Geometry {
                    origin: Origin::User,
                    points: vec![
                        Point { x: 0.0, y: 0.0 },
                        Point { x: 100.0, y: 0.0 },
                        Point { x: 100.0, y: 100.0 },
                        Point { x: 0.0, y: 100.0 },
                    ],
                },
                Some("bubble 1".to_owned()),
            )?;
            edit.set(
                bubble_1,
                &BalloonClassification {
                    origin: Origin::User,
                    shape: BalloonShape::NormalSpeech,
                    confidence: 0.98,
                },
            )?;
            bubble_1_id = Some(bubble_1);

            let bubble_2 = edit.add_analysis_region::<BubbleRegion>(
                page,
                At::End,
                &Geometry {
                    origin: Origin::User,
                    points: vec![
                        Point { x: 200.0, y: 200.0 },
                        Point { x: 300.0, y: 200.0 },
                        Point { x: 300.0, y: 300.0 },
                        Point { x: 200.0, y: 300.0 },
                    ],
                },
                Some("bubble 2".to_owned()),
            )?;
            edit.set(
                bubble_2,
                &BalloonClassification {
                    origin: Origin::User,
                    shape: BalloonShape::Shout,
                    confidence: 0.92,
                },
            )?;
            bubble_2_id = Some(bubble_2);

            // 5. Connect relations
            edit.relate::<FlowsIn>(layer_1, bubble_1)?;
            edit.relate::<FlowsIn>(layer_2, bubble_2)?;

            Ok(())
        })
        .unwrap();

    assert!(!patch.is_empty());

    // Prune against base snapshot (which has NO pages or entities)
    let (pruned, diagnostics) = patch.prune_missing_with_diagnostics(&base_snap);

    assert!(
        diagnostics.is_empty(),
        "Accumulating state should NOT drop any valid chained operations: {diagnostics:?}"
    );
    assert!(!pruned.is_empty());

    // Commit and verify state integrity
    let commit = session.commit(pruned).await.unwrap();
    let snap = commit.snapshot;

    let p = page_id.unwrap();
    let l1 = layer_1_id.unwrap();
    let l2 = layer_2_id.unwrap();
    let b1 = bubble_1_id.unwrap();
    let b2 = bubble_2_id.unwrap();

    assert!(snap.page(p).is_ok());
    assert_eq!(snap.text_layers().unwrap().len(), 2);

    let l1_ref = snap.text_layer(l1).unwrap();
    assert_eq!(l1_ref.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));

    let b1_class = snap.component::<BalloonClassification>(b1).unwrap().unwrap();
    assert_eq!(b1_class.shape, BalloonShape::NormalSpeech);

    let b2_class = snap.component::<BalloonClassification>(b2).unwrap().unwrap();
    assert_eq!(b2_class.shape, BalloonShape::Shout);

    let l1_flows = snap.relations_from_as::<FlowsIn>(l1).collect::<Vec<_>>();
    assert_eq!(l1_flows.len(), 1);
    assert_eq!(l1_flows[0].value().target, b1);

    let l2_flows = snap.relations_from_as::<FlowsIn>(l2).collect::<Vec<_>>();
    assert_eq!(l2_flows.len(), 1);
    assert_eq!(l2_flows[0].value().target, b2);
}

#[tokio::test]
async fn test_c2_missing_dependencies_cascade_prune_cleanly() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            page_id = Some(edit.add_page(sample_page("p_del"), At::End)?);
            Ok(())
        })
        .unwrap();
    let base = session.commit(setup).await.unwrap().snapshot;
    let page = page_id.unwrap();

    // Create a patch that adds content, layer, and relation on the page
    let patch = base
        .patch(|edit| {
            let content = edit.add_text_content(page, At::End)?;
            let layer = edit.add_text_layer(
                page,
                At::End,
                content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            let bubble = edit.add_analysis_region::<BubbleRegion>(
                page,
                At::End,
                &Geometry {
                    origin: Origin::User,
                    points: vec![
                        Point { x: 0.0, y: 0.0 },
                        Point { x: 10.0, y: 0.0 },
                        Point { x: 10.0, y: 10.0 },
                        Point { x: 0.0, y: 10.0 },
                    ],
                },
                None,
            )?;
            edit.relate::<FlowsIn>(layer, bubble)?;
            Ok(())
        })
        .unwrap();

    // Now simulate an alternative history where the page was deleted
    let delete_page_patch = base
        .patch(|edit| edit.remove_entity(page, RemovePolicy::Cascade))
        .unwrap();
    let deleted_page_snapshot = session.commit(delete_page_patch).await.unwrap().snapshot;

    // Pruning against deleted_page_snapshot must prune EVERYTHING in patch
    let (pruned, diagnostics) = patch.prune_missing_with_diagnostics(&deleted_page_snapshot);

    assert!(
        !diagnostics.is_empty(),
        "Pruning against deleted root must report diagnostics"
    );
    assert!(
        pruned.is_empty(),
        "All dependent operations must be pruned when parent page is deleted"
    );
}

#[tokio::test]
async fn test_c2_invalid_entity_id_rejection_in_edit() {
    let session = Session::memory().await.unwrap();
    let invalid_id = EntityId::new();

    // Calling remove_entity on a non-existent ID must return EntityNotFound
    let err = session
        .snapshot()
        .patch(|edit| edit.remove_entity(invalid_id, RemovePolicy::RejectNonEmpty));
    assert!(err.is_err(), "remove_entity on missing ID must error");
}

#[tokio::test]
async fn test_c2_multi_page_and_hierarchy_chaining() {
    let mut session = Session::memory().await.unwrap();
    let base_snap = session.snapshot();

    // In a single patch, create 2 pages, with entities in each, and verify accumulation
    let patch = base_snap
        .patch(|edit| {
            let p1 = edit.add_page(sample_page("page_1"), At::End)?;
            let p2 = edit.add_page(sample_page("page_2"), At::End)?;

            let c1 = edit.add_text_content(p1, At::End)?;
            let c2 = edit.add_text_content(p2, At::End)?;

            edit.set(c1, &sample_source("Page 1 text"))?;
            edit.set(c2, &sample_source("Page 2 text"))?;

            let l1 = edit.add_text_layer(
                p1,
                At::End,
                c1,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;

            let l2 = edit.add_text_layer(
                p2,
                At::End,
                c2,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;

            assert_ne!(l1, l2);
            Ok(())
        })
        .unwrap();

    let (pruned, diagnostics) = patch.prune_missing_with_diagnostics(&base_snap);
    assert!(diagnostics.is_empty());
    assert!(!pruned.is_empty());

    let commit = session.commit(pruned).await.unwrap();
    assert_eq!(commit.snapshot.pages().len(), 2);
    assert_eq!(commit.snapshot.text_layers().unwrap().len(), 2);
}

// =========================================================================
// C3: Empirical Verification of PruneDiagnostic External Visibility & Usability
// =========================================================================

#[tokio::test]
async fn test_c3_prune_diagnostic_external_consumer_direct_import_and_pattern_match() {
    // 1. Direct explicit import from crate root (no wildcard)
    use koharu_scene::{EntityId, PruneDiagnostic, RelationId, RemovePolicy, Session};

    // 2. Verify all 4 variants are constructible, cloneable, debuggable, and equality-comparable
    let dummy_entity = EntityId::new();
    let dummy_relation = RelationId::new();
    let sample_diagnostics = vec![
        PruneDiagnostic::MissingEntity {
            operation_index: 0,
            entity: dummy_entity,
        },
        PruneDiagnostic::MissingRelation {
            operation_index: 1,
            relation: dummy_relation,
        },
        PruneDiagnostic::MissingPage {
            operation_index: 2,
            page: dummy_entity,
        },
        PruneDiagnostic::ApplyFailed {
            operation_index: 3,
            error: "test conflict".to_string(),
        },
    ];

    for diag in &sample_diagnostics {
        let cloned = diag.clone();
        assert_eq!(diag, &cloned);
        let debug_str = format!("{diag:?}");
        assert!(!debug_str.is_empty());

        // Exhaustive pattern matching on external import
        match diag {
            PruneDiagnostic::MissingEntity {
                operation_index,
                entity,
            } => {
                assert_eq!(*operation_index, 0);
                assert_eq!(*entity, dummy_entity);
            }
            PruneDiagnostic::MissingRelation {
                operation_index,
                relation,
            } => {
                assert_eq!(*operation_index, 1);
                assert_eq!(*relation, dummy_relation);
            }
            PruneDiagnostic::MissingPage {
                operation_index,
                page,
            } => {
                assert_eq!(*operation_index, 2);
                assert_eq!(*page, dummy_entity);
            }
            PruneDiagnostic::ApplyFailed {
                operation_index,
                error,
            } => {
                assert_eq!(*operation_index, 3);
                assert_eq!(error, "test conflict");
            }
        }
    }

    // 3. Empirically verify real diagnostics emitted by prune_missing_with_diagnostics
    let mut session = Session::memory().await.unwrap();
    let mut page1_id_opt = None;
    let p1 = session
        .snapshot()
        .patch(|edit| {
            let pid = edit.add_page(sample_page("p1"), At::End)?;
            page1_id_opt = Some(pid);
            Ok(())
        })
        .unwrap();
    let _snap1 = session.commit(p1).await.unwrap().snapshot;
    let page1_id = page1_id_opt.unwrap();

    // In a branch from snap1, build a patch that operates on page1
    let mut c1_id_opt = None;
    let patch_to_prune = {
        let snap = session.snapshot();
        let p_add = snap
            .patch(|edit| {
                let c = edit.add_text_content(page1_id, At::End)?;
                edit.set(c, &sample_source("hello"))?;
                c1_id_opt = Some(c);
                Ok(())
            })
            .unwrap();
        let commit = session.commit(p_add).await.unwrap();
        let c1_id = c1_id_opt.unwrap();

        // Create patch modifying c1 and page1
        commit
            .snapshot
            .patch(|edit| {
                edit.set(c1_id, &sample_source("updated"))?;
                edit.remove_entity(page1_id, RemovePolicy::Cascade)?;
                Ok(())
            })
            .unwrap()
    };
    let c1_id = c1_id_opt.unwrap();

    // Now delete page1 on session
    let delete_page = session
        .snapshot()
        .patch(|edit| edit.remove_entity(page1_id, RemovePolicy::Cascade))
        .unwrap();
    let snap_after_deletion = session.commit(delete_page).await.unwrap().snapshot;

    // Prune patch against snap_after_deletion where page1 no longer exists
    let (pruned, diagnostics) = patch_to_prune.prune_missing_with_diagnostics(&snap_after_deletion);
    assert!(pruned.is_empty());
    assert!(!diagnostics.is_empty());

    // Pattern-match and verify actual diagnostics produced
    let mut observed_missing_entity = false;
    let mut observed_missing_page = false;

    for diag in diagnostics {
        match diag {
            PruneDiagnostic::MissingEntity {
                operation_index,
                entity,
            } => {
                observed_missing_entity = true;
                assert!(entity == c1_id || entity == page1_id);
                println!("Observed MissingEntity at op {operation_index}: {entity:?}");
            }
            PruneDiagnostic::MissingPage {
                operation_index,
                page,
            } => {
                observed_missing_page = true;
                assert_eq!(page, page1_id);
                println!("Observed MissingPage at op {operation_index}: {page:?}");
            }
            PruneDiagnostic::MissingRelation {
                operation_index,
                relation,
            } => {
                println!("Observed MissingRelation at op {operation_index}: {relation:?}");
            }
            PruneDiagnostic::ApplyFailed {
                operation_index,
                error,
            } => {
                println!("Observed ApplyFailed at op {operation_index}: {error}");
            }
        }
    }

    assert!(
        observed_missing_entity,
        "Expected at least one MissingEntity diagnostic for pruned operations"
    );
    assert!(
        observed_missing_page,
        "Expected at least one MissingPage diagnostic for pruned operations"
    );
}

#[tokio::test]
async fn test_c3_prune_diagnostic_missing_relation_real_emission() {
    use koharu_scene::{BubbleRegion, FlowsIn, PruneDiagnostic, Session};

    let mut session = Session::memory().await.unwrap();
    let mut rel_id = None;

    let init_patch = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(sample_page("p1"), At::End)?;

            let c = edit.add_text_content(p, At::End)?;
            edit.set(c, &sample_source("hello"))?;

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

            let b = edit.add_analysis_region::<BubbleRegion>(
                p,
                At::End,
                &Geometry {
                    origin: Origin::User,
                    points: vec![
                        Point { x: 0.0, y: 0.0 },
                        Point { x: 10.0, y: 0.0 },
                        Point { x: 10.0, y: 10.0 },
                    ],
                },
                None,
            )?;

            let r = edit.relate::<FlowsIn>(l, b)?;
            rel_id = Some(r);
            Ok(())
        })
        .unwrap();

    let commit1 = session.commit(init_patch).await.unwrap();
    let r_id = rel_id.unwrap();

    // Create a patch that removes the relation
    let remove_rel_patch = commit1
        .snapshot
        .patch(|edit| {
            edit.remove_relation(r_id)?;
            Ok(())
        })
        .unwrap();

    // Now remove the relation on the main session
    let delete_rel = session
        .snapshot()
        .patch(|edit| {
            edit.remove_relation(r_id)?;
            Ok(())
        })
        .unwrap();
    let snap_after_rel_del = session.commit(delete_rel).await.unwrap().snapshot;

    // Prune remove_rel_patch against snap_after_rel_del (relation already gone)
    let (pruned, diagnostics) = remove_rel_patch.prune_missing_with_diagnostics(&snap_after_rel_del);
    assert!(pruned.is_empty());
    assert_eq!(diagnostics.len(), 1);

    // Direct pattern match and verify MissingRelation
    match &diagnostics[0] {
        PruneDiagnostic::MissingRelation {
            operation_index,
            relation,
        } => {
            assert_eq!(*operation_index, 0);
            assert_eq!(*relation, r_id);
            println!("Empirically verified MissingRelation diagnostic: op {operation_index}, relation {relation:?}");
        }
        other => panic!("Expected MissingRelation, got {other:?}"),
    }
}



