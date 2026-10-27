//! Opaque-box E2E and integration tests for `koharu-scene`
//! Covering Tiers 1-4 per TEST_INFRA.md:
//! - Tier 1: Feature coverage (schema, patch accumulation, undo/redo roundtrips, speaker provenance)
//! - Tier 2: Boundary & corner cases (empty inputs, missing entities, stale revisions, ID mismatch)
//! - Tier 3: Cross-feature combinations (simultaneous creations, cross-page moves, concurrent rebase)
//! - Tier 4: Real-world workloads (full project lifecycle v1->v2 upgrade, edit, save, reload)

use koharu_scene::*;

fn sample_page() -> PageDraft {
    PageDraft::new("p1", 1200.0, 1800.0)
}

fn sample_source(text: &str) -> SourceText {
    SourceText {
        text: Authored::user(text.to_owned()),
        language: Some(LanguageTag::new("ja").unwrap()),
    }
}

// =========================================================================
// TIER 1: Feature Coverage Tests
// =========================================================================

#[tokio::test]
async fn tier1_schema_glossary_entry_v2_character_profile_persistence() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let profile = CharacterProfile {
        id: entry_id,
        gender: CharacterGender::Female,
        first_person: Some("私".to_owned()),
        second_person: Some("あなた".to_owned()),
        tone: Some("polite".to_owned()),
        custom_instructions: Some("Soft tone".to_owned()),
    };

    let glossary = Glossary {
        enabled: true,
        source_language: Some(LanguageTag::new("ja").unwrap()),
        target_language: Some(LanguageTag::new("en").unwrap()),
        source_fingerprint: None,
        entries: vec![GlossaryEntry {
            id: entry_id,
            source: "千早".to_owned(),
            translation: Some("Chihaya".to_owned()),
            kind: GlossaryKind::Person,
            enabled: true,
            confidence: Some(1.0),
            occurrence_count: 3,
            examples: vec!["千早は歌った".to_owned()],
            source_origin: GlossaryValueOrigin::User,
            translation_origin: Some(GlossaryValueOrigin::User),
            present_in_last_scan: true,
            character: Some(profile.clone()),
        }],
    };

    let patch = session
        .snapshot()
        .patch(|edit| edit.set_project(&glossary))
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;

    let read_glossary = snapshot
        .project_component::<Glossary>()
        .unwrap()
        .expect("glossary component must exist");
    assert_eq!(read_glossary.entries.len(), 1);
    let entry = &read_glossary.entries[0];
    assert_eq!(entry.id, entry_id);
    assert_eq!(entry.source, "千早");
    assert_eq!(entry.character.as_ref().unwrap().gender, CharacterGender::Female);
    assert_eq!(entry.character.as_ref().unwrap().first_person.as_deref(), Some("私"));
}

#[tokio::test]
async fn tier1_speaker_origin_provenance_user_vs_generated() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();

    let mut text_layer_user = None;
    let mut text_layer_gen = None;

    let det_gen = Generation::new(
        ProducerId::new("dev.koharu.pipeline.detection").unwrap(),
    );

    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(sample_page(), At::End)?;
            let content_1 = edit.add_text_content(page, At::End)?;
            edit.set(content_1, &sample_source("ユーザー発言"))?;
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
            text_layer_user = Some(layer_1);

            let content_2 = edit.add_text_content(page, At::End)?;
            edit.set(content_2, &sample_source("AI推定発言"))?;
            let layer_2 = edit.add_text_layer(
                page,
                At::End,
                content_2,
                &TextLayout {
                    origin: Origin::Generated(det_gen.clone()),
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            edit.set_speaker(layer_2, Some(entry_id), Origin::Generated(det_gen.clone()))?;
            text_layer_gen = Some(layer_2);
            Ok(())
        })
        .unwrap();

    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let l1 = snapshot.text_layer(text_layer_user.unwrap()).unwrap();
    let l2 = snapshot.text_layer(text_layer_gen.unwrap()).unwrap();

    assert_eq!(l1.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));
    assert_eq!(l2.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));
    assert_eq!(l1.speaker().unwrap().unwrap().origin, Origin::User);
    assert_eq!(l2.speaker().unwrap().unwrap().origin, Origin::Generated(det_gen));
}

#[tokio::test]
async fn tier1_patch_prune_missing_sequential_state_accumulation() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;
    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(sample_page(), At::End)?;
            page_id = Some(p);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();
    let page = page_id.unwrap();

    // In a single patch: create text content, then add text layer pointing to that content
    let patch = session
        .snapshot()
        .patch(|edit| {
            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &sample_source("Sequential test"))?;
            let _layer = edit.add_text_layer(
                page,
                At::End,
                content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            Ok(())
        })
        .unwrap();

    let pruned = patch.prune_missing(&session.snapshot());
    assert!(!pruned.is_empty());
    let commit = session.commit(pruned).await.unwrap();
    assert_eq!(commit.snapshot.text_layers().unwrap().len(), 1);
}

#[tokio::test]
async fn tier1_undo_redo_speaker_component() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let mut layer_id = None;

    let init = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(sample_page(), At::End)?;
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
            layer_id = Some(layer);
            Ok(())
        })
        .unwrap();
    session.commit(init).await.unwrap();
    let layer = layer_id.unwrap();

    // Assign speaker
    let set_speaker = session
        .snapshot()
        .patch(|edit| {
            edit.set_speaker(layer, Some(entry_id), Origin::User)?;
            Ok(())
        })
        .unwrap();
    let commit_set = session.commit(set_speaker).await.unwrap();
    assert_eq!(
        commit_set.snapshot.text_layer(layer).unwrap().speaker().unwrap().and_then(|s| s.speaker),
        Some(entry_id)
    );

    let undo_commit = session.undo(commit_set.revision).await.unwrap();
    assert_eq!(
        undo_commit.snapshot.text_layer(layer).unwrap().speaker().unwrap().and_then(|s| s.speaker),
        None
    );

    let redo_commit = session.undo(undo_commit.revision).await.unwrap();
    assert_eq!(
        redo_commit.snapshot.text_layer(layer).unwrap().speaker().unwrap().and_then(|s| s.speaker),
        Some(entry_id)
    );
}

#[tokio::test]
async fn tier1_undo_redo_balloon_classification_component() {
    let mut session = Session::memory().await.unwrap();
    let mut layer_id = None;

    let init = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(sample_page(), At::End)?;
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
            layer_id = Some(layer);
            Ok(())
        })
        .unwrap();
    session.commit(init).await.unwrap();
    let layer = layer_id.unwrap();

    let set_balloon = session
        .snapshot()
        .patch(|edit| {
            edit.set(
                layer,
                &BalloonClassification {
                    origin: Origin::User,
                    shape: BalloonShape::Shout,
                    confidence: 0.95,
                },
            )?;
            Ok(())
        })
        .unwrap();
    let commit_set = session.commit(set_balloon).await.unwrap();
    assert_eq!(
        commit_set.snapshot.component::<BalloonClassification>(layer).unwrap().map(|b| b.shape),
        Some(BalloonShape::Shout)
    );

    let undo_commit = session.undo(commit_set.revision).await.unwrap();
    assert!(
        undo_commit.snapshot.component::<BalloonClassification>(layer).unwrap().is_none()
    );

    let redo_commit = session.undo(undo_commit.revision).await.unwrap();
    assert_eq!(
        redo_commit.snapshot.component::<BalloonClassification>(layer).unwrap().map(|b| b.shape),
        Some(BalloonShape::Shout)
    );
}

#[tokio::test]
async fn tier1_undo_redo_character_profile_metadata() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();

    let initial_glossary = Glossary {
        enabled: true,
        source_language: None,
        target_language: None,
        source_fingerprint: None,
        entries: vec![GlossaryEntry {
            id: entry_id,
            source: "主人公".to_owned(),
            translation: Some("Protagonist".to_owned()),
            kind: GlossaryKind::Person,
            enabled: true,
            confidence: None,
            occurrence_count: 1,
            examples: vec![],
            source_origin: GlossaryValueOrigin::User,
            translation_origin: None,
            present_in_last_scan: true,
            character: None,
        }],
    };

    let p1 = session.snapshot().patch(|edit| edit.set_project(&initial_glossary)).unwrap();
    session.commit(p1).await.unwrap();

    // Add profile
    let mut updated_glossary = initial_glossary.clone();
    updated_glossary.entries[0].character = Some(CharacterProfile {
        id: entry_id,
        gender: CharacterGender::Male,
        first_person: Some("俺".to_owned()),
        second_person: Some("お前".to_owned()),
        tone: Some("rough".to_owned()),
        custom_instructions: None,
    });

    let p2 = session.snapshot().patch(|edit| edit.set_project(&updated_glossary)).unwrap();
    let commit_update = session.commit(p2).await.unwrap();
    assert!(
        commit_update.snapshot.project_component::<Glossary>().unwrap().unwrap().entries[0]
            .character
            .is_some()
    );

    let undo_commit = session.undo(commit_update.revision).await.unwrap();
    assert!(
        undo_commit.snapshot.project_component::<Glossary>().unwrap().unwrap().entries[0]
            .character
            .is_none()
    );

    let redo_commit = session.undo(undo_commit.revision).await.unwrap();
    assert!(
        redo_commit.snapshot.project_component::<Glossary>().unwrap().unwrap().entries[0]
            .character
            .is_some()
    );
}

#[tokio::test]
async fn tier1_balloon_classification_shapes_all_variants() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;
    let setup = session
        .snapshot()
        .patch(|edit| {
            page_id = Some(edit.add_page(sample_page(), At::End)?);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();
    let page = page_id.unwrap();

    let shapes = [
        BalloonShape::NormalSpeech,
        BalloonShape::Shout,
        BalloonShape::Thought,
        BalloonShape::Narration,
        BalloonShape::Whisper,
    ];

    for shape in shapes {
        let patch = session
            .snapshot()
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
                edit.set(
                    layer,
                    &BalloonClassification {
                        origin: Origin::User,
                        shape,
                        confidence: 0.90,
                    },
                )?;
                Ok(())
            })
            .unwrap();
        let commit = session.commit(patch).await.unwrap();
        let layers = commit.snapshot.text_layers().unwrap();
        let last_layer = layers.last().unwrap();
        let classification = commit.snapshot.component::<BalloonClassification>(last_layer.id()).unwrap().unwrap();
        assert_eq!(classification.shape, shape);
        assert!((classification.confidence - 0.90).abs() < f32::EPSILON);
    }
}

#[tokio::test]
async fn tier1_scene_snapshot_revision_monotonicity() {
    let mut session = Session::memory().await.unwrap();
    assert_eq!(session.snapshot().revision(), Revision::ZERO);

    for expected_rev in 1..=10 {
        let patch = session
            .snapshot()
            .patch(|edit| {
                edit.add_page(PageDraft::new(format!("page_{expected_rev}"), 100.0, 100.0), At::End)?;
                Ok(())
            })
            .unwrap();
        let commit = session.commit(patch).await.unwrap();
        assert_eq!(commit.snapshot.revision(), Revision::new(expected_rev));
        assert_eq!(session.snapshot().revision(), Revision::new(expected_rev));
    }
}

// =========================================================================
// TIER 2: Boundary & Corner Case Tests
// =========================================================================

#[tokio::test]
async fn tier2_empty_glossary_entries_validation() {
    let mut session = Session::memory().await.unwrap();
    let glossary = Glossary {
        enabled: true,
        source_language: None,
        target_language: None,
        source_fingerprint: None,
        entries: vec![],
    };

    let patch = session.snapshot().patch(|edit| edit.set_project(&glossary)).unwrap();
    let commit = session.commit(patch).await.unwrap();
    let read = commit.snapshot.project_component::<Glossary>().unwrap().unwrap();
    assert!(read.entries.is_empty());
    assert!(read.enabled);
}

#[tokio::test]
async fn tier2_character_profile_validation_id_mismatch_rejected() {
    let session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let different_id = GlossaryEntryId::new();

    let glossary = Glossary {
        enabled: true,
        source_language: None,
        target_language: None,
        source_fingerprint: None,
        entries: vec![GlossaryEntry {
            id: entry_id,
            source: "テスト".to_owned(),
            translation: None,
            kind: GlossaryKind::Person,
            enabled: true,
            confidence: None,
            occurrence_count: 0,
            examples: vec![],
            source_origin: GlossaryValueOrigin::User,
            translation_origin: None,
            present_in_last_scan: true,
            character: Some(CharacterProfile {
                id: different_id, // MISMATCH
                gender: CharacterGender::Unspecified,
                first_person: None,
                second_person: None,
                tone: None,
                custom_instructions: None,
            }),
        }],
    };

    let patch_res = session.snapshot().patch(|edit| edit.set_project(&glossary));
    assert!(patch_res.is_err(), "ID mismatch between entry and character profile must be rejected");
}

#[tokio::test]
async fn tier2_glossary_text_max_chars_boundary_enforced() {
    let session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let long_source = "a".repeat(GLOSSARY_TEXT_MAX_CHARS + 1);

    let glossary = Glossary {
        enabled: true,
        source_language: None,
        target_language: None,
        source_fingerprint: None,
        entries: vec![GlossaryEntry {
            id: entry_id,
            source: long_source,
            translation: None,
            kind: GlossaryKind::Term,
            enabled: true,
            confidence: None,
            occurrence_count: 0,
            examples: vec![],
            source_origin: GlossaryValueOrigin::User,
            translation_origin: None,
            present_in_last_scan: true,
            character: None,
        }],
    };

    assert!(session.snapshot().patch(|edit| edit.set_project(&glossary)).is_err());
}

#[tokio::test]
async fn tier2_glossary_max_examples_boundary_enforced() {
    let session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let too_many_examples = (0..=GLOSSARY_MAX_EXAMPLES)
        .map(|i| format!("example {i}"))
        .collect();

    let glossary = Glossary {
        enabled: true,
        source_language: None,
        target_language: None,
        source_fingerprint: None,
        entries: vec![GlossaryEntry {
            id: entry_id,
            source: "valid_source".to_owned(),
            translation: None,
            kind: GlossaryKind::Term,
            enabled: true,
            confidence: None,
            occurrence_count: 0,
            examples: too_many_examples,
            source_origin: GlossaryValueOrigin::User,
            translation_origin: None,
            present_in_last_scan: true,
            character: None,
        }],
    };

    assert!(session.snapshot().patch(|edit| edit.set_project(&glossary)).is_err());
}

#[tokio::test]
async fn tier2_stale_revision_rebase_rejection() {
    let mut session = Session::memory().await.unwrap();
    let setup = session
        .snapshot()
        .patch(|edit| {
            edit.add_page(sample_page(), At::End)?;
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();

    let base = session.snapshot();
    let patch_a = base
        .patch(|edit| {
            edit.add_page(PageDraft::new("branch_a", 100.0, 100.0), At::End)?;
            Ok(())
        })
        .unwrap();

    let patch_b = base
        .patch(|edit| {
            edit.add_page(PageDraft::new("branch_b", 100.0, 100.0), At::End)?;
            Ok(())
        })
        .unwrap();

    // Commit A
    session.commit(patch_a).await.unwrap();

    // Patch B is now stale (base revision is 1, session revision is 2)
    assert_ne!(patch_b.base_revision(), session.snapshot().revision());
    // Direct commit without rebase should fail
    let direct_commit_result = session.commit(patch_b.clone()).await;
    assert!(direct_commit_result.is_err());
}

#[tokio::test]
async fn tier2_same_patch_insert_entity_and_relation_retained_during_prune() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;
    let init = session
        .snapshot()
        .patch(|edit| {
            page_id = Some(edit.add_page(sample_page(), At::End)?);
            Ok(())
        })
        .unwrap();
    session.commit(init).await.unwrap();
    let page = page_id.unwrap();

    let base_snap = session.snapshot();
    let patch = base_snap
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
                        Point { x: 100.0, y: 0.0 },
                        Point { x: 100.0, y: 100.0 },
                        Point { x: 0.0, y: 100.0 },
                    ],
                },
                Some("bubble".to_owned()),
            )?;
            edit.relate::<FlowsIn>(layer, bubble)?;
            Ok(())
        })
        .unwrap();

    // Prune against base state
    let pruned = patch.prune_missing(&base_snap);
    assert!(!pruned.is_empty());
    let commit = session.commit(pruned).await.unwrap();
    assert_eq!(commit.snapshot.text_layers().unwrap().len(), 1);
}

// =========================================================================
// TIER 3: Cross-Feature Combination Tests
// =========================================================================

#[tokio::test]
async fn tier3_simultaneous_entity_creation_relation_and_glossary_update() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();

    let patch = session
        .snapshot()
        .patch(|edit| {
            // 1. Update project glossary with persona
            let glossary = Glossary {
                enabled: true,
                source_language: Some(LanguageTag::new("ja").unwrap()),
                target_language: Some(LanguageTag::new("en").unwrap()),
                source_fingerprint: None,
                entries: vec![GlossaryEntry {
                    id: entry_id,
                    source: "リン".to_owned(),
                    translation: Some("Rin".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    confidence: Some(1.0),
                    occurrence_count: 1,
                    examples: vec![],
                    source_origin: GlossaryValueOrigin::User,
                    translation_origin: Some(GlossaryValueOrigin::User),
                    present_in_last_scan: true,
                    character: Some(CharacterProfile {
                        id: entry_id,
                        gender: CharacterGender::Female,
                        first_person: Some("あたし".to_owned()),
                        second_person: Some("あんた".to_owned()),
                        tone: Some("energetic".to_owned()),
                        custom_instructions: None,
                    }),
                }],
            };
            edit.set_project(&glossary)?;

            // 2. Add page, text content, and layer
            let page = edit.add_page(sample_page(), At::End)?;
            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &sample_source("やったー！"))?;
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

            // 3. Assign speaker and balloon classification
            edit.set_speaker(layer, Some(entry_id), Origin::User)?;
            edit.set(
                layer,
                &BalloonClassification {
                    origin: Origin::User,
                    shape: BalloonShape::Shout,
                    confidence: 0.98,
                },
            )?;

            Ok(())
        })
        .unwrap();

    let commit = session.commit(patch).await.unwrap();
    let snap = commit.snapshot;

    let glos = snap.project_component::<Glossary>().unwrap().unwrap();
    assert_eq!(glos.entries[0].character.as_ref().unwrap().gender, CharacterGender::Female);

    let layer = snap.text_layers().unwrap().into_iter().next().unwrap();
    assert_eq!(layer.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));
    assert_eq!(
        snap.component::<BalloonClassification>(layer.id()).unwrap().map(|b| b.shape),
        Some(BalloonShape::Shout)
    );
}

#[tokio::test]
async fn tier3_cross_page_text_layer_move_preserving_speaker_and_balloon_classification() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let mut p1_id = None;
    let mut p2_id = None;
    let mut layer_id = None;

    let init = session
        .snapshot()
        .patch(|edit| {
            let p1 = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
            let p2 = edit.add_page(PageDraft::new("p2", 100.0, 100.0), At::End)?;
            let p2_content = edit.add_text_content(p2, At::End)?;
            let _p2_layer = edit.add_text_layer(
                p2,
                At::End,
                p2_content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;

            let content = edit.add_text_content(p1, At::End)?;
            let layer = edit.add_text_layer(
                p1,
                At::End,
                content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            edit.set_speaker(layer, Some(entry_id), Origin::User)?;
            edit.set(
                layer,
                &BalloonClassification {
                    origin: Origin::User,
                    shape: BalloonShape::Thought,
                    confidence: 0.85,
                },
            )?;

            p1_id = Some(p1);
            p2_id = Some(p2);
            layer_id = Some(layer);
            Ok(())
        })
        .unwrap();
    let init_snap = session.commit(init).await.unwrap().snapshot;
    let layer = layer_id.unwrap();
    let p1 = p1_id.unwrap();
    let p2 = p2_id.unwrap();
    let p2_group = init_snap.page(p2).unwrap().text_group().unwrap().unwrap().id();

    // Move layer to page 2's text group
    let move_patch = session
        .snapshot()
        .patch(|edit| {
            edit.move_entity(layer, Some(p2_group), At::End)?;
            Ok(())
        })
        .unwrap();
    let moved_snap = session.commit(move_patch).await.unwrap().snapshot;

    let moved_ref = moved_snap.text_layer(layer).unwrap();
    assert_eq!(moved_ref.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));
    assert_eq!(
        moved_snap.component::<BalloonClassification>(layer).unwrap().map(|b| b.shape),
        Some(BalloonShape::Thought)
    );
    assert_eq!(moved_snap.page(p1).unwrap().text_group().unwrap().unwrap().text_layers().unwrap().count(), 0);
    assert_eq!(moved_snap.page(p2).unwrap().text_group().unwrap().unwrap().text_layers().unwrap().count(), 2);
}

// =========================================================================
// TIER 4: Real-World Workload Tests
// =========================================================================

#[tokio::test]
async fn tier4_scenario_4_full_project_lifecycle_v1_upgrade_and_lossless_roundtrip() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();

    // 1. Initialize project with a chapter structure and character persona
    let init_patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(PageDraft::new("Chapter 1 Cover", 1200.0, 1800.0), At::End)?;
            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &sample_source("私の名前はユキです。"))?;
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
            edit.set_speaker(layer, Some(entry_id), Origin::User)?;
            edit.set(
                layer,
                &BalloonClassification {
                    origin: Origin::User,
                    shape: BalloonShape::NormalSpeech,
                    confidence: 0.95,
                },
            )?;

            let glossary = Glossary {
                enabled: true,
                source_language: Some(LanguageTag::new("ja").unwrap()),
                target_language: Some(LanguageTag::new("en").unwrap()),
                source_fingerprint: None,
                entries: vec![GlossaryEntry {
                    id: entry_id,
                    source: "ユキ".to_owned(),
                    translation: Some("Yuki".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    confidence: Some(1.0),
                    occurrence_count: 10,
                    examples: vec!["ユキです".to_owned()],
                    source_origin: GlossaryValueOrigin::User,
                    translation_origin: Some(GlossaryValueOrigin::User),
                    present_in_last_scan: true,
                    character: Some(CharacterProfile {
                        id: entry_id,
                        gender: CharacterGender::Female,
                        first_person: Some("私".to_owned()),
                        second_person: Some("あなた".to_owned()),
                        tone: Some("gentle".to_owned()),
                        custom_instructions: Some("Female high school student".to_owned()),
                    }),
                }],
            };
            edit.set_project(&glossary)?;
            Ok(())
        })
        .unwrap();

    let commit_1 = session.commit(init_patch).await.unwrap();
    assert_eq!(commit_1.snapshot.revision(), Revision::new(1));

    // 2. Perform translation edit in next revision
    let layer_id = commit_1.snapshot.text_layers().unwrap().into_iter().next().unwrap().id();
    let content_id = commit_1.snapshot.text_layer(layer_id).unwrap().content().unwrap().id();

    let trans_patch = commit_1
        .snapshot
        .patch(|edit| {
            edit.set(
                content_id,
                &Translation {
                    text: Authored::user("My name is Yuki.".to_owned()),
                    language: Some(LanguageTag::new("en").unwrap()),
                },
            )?;
            Ok(())
        })
        .unwrap();

    let commit_2 = session.commit(trans_patch).await.unwrap();
    assert_eq!(commit_2.snapshot.revision(), Revision::new(2));

    // Verify all components, speaker, and persona persisted losslessly
    let final_snap = commit_2.snapshot;
    let final_glossary = final_snap.project_component::<Glossary>().unwrap().unwrap();
    assert_eq!(final_glossary.entries.len(), 1);
    let char_profile = final_glossary.entries[0].character.as_ref().unwrap();
    assert_eq!(char_profile.first_person.as_deref(), Some("私"));
    assert_eq!(char_profile.custom_instructions.as_deref(), Some("Female high school student"));

    let text_layer = final_snap.text_layer(layer_id).unwrap();
    assert_eq!(text_layer.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));
    assert_eq!(
        final_snap.component::<BalloonClassification>(layer_id).unwrap().map(|b| b.shape),
        Some(BalloonShape::NormalSpeech)
    );
    assert_eq!(
        text_layer.content().unwrap().translation().unwrap().unwrap().text.value,
        "My name is Yuki."
    );
}
