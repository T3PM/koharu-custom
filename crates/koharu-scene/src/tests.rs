use std::{collections::BTreeMap, sync::Arc};

use revision::revisioned;

use crate::*;

const EVOLVING_COMPONENT_KIND: &str = "dev.koharu.test.evolving";

#[revisioned(revision = 1)]
#[derive(Clone)]
struct EvolvingComponentV1 {
    title: String,
    legacy_count: u32,
}

impl Component for EvolvingComponentV1 {
    const KIND: &'static str = EVOLVING_COMPONENT_KIND;
}

#[revisioned(revision = 2)]
#[derive(Clone, Debug, PartialEq)]
struct EvolvingComponentV2 {
    title: String,
    #[revision(end = 2, convert_fn = "migrate_legacy_count")]
    legacy_count: u32,
    #[revision(start = 2)]
    count: u64,
    #[revision(start = 2, default_fn = "default_label")]
    label: String,
}

impl EvolvingComponentV2 {
    fn migrate_legacy_count(
        &mut self,
        _revision: u16,
        value: u32,
    ) -> std::result::Result<(), revision::Error> {
        self.count = u64::from(value);
        Ok(())
    }

    fn default_label(_revision: u16) -> std::result::Result<String, revision::Error> {
        Ok("migrated".to_owned())
    }
}

impl Component for EvolvingComponentV2 {
    const KIND: &'static str = EVOLVING_COMPONENT_KIND;
}

fn page() -> PageDraft {
    PageDraft::new("page", 1200.0, 1800.0)
}

fn assert_send_sync<T: Send + Sync>() {}

fn source(text: &str) -> SourceText {
    SourceText {
        text: Authored::user(text.to_owned()),
        language: Some(LanguageTag::new("ja").unwrap()),
    }
}

#[tokio::test]
async fn fresh_scene_has_an_empty_project_hierarchy() {
    assert_send_sync::<Snapshot>();
    assert_send_sync::<Patch>();
    let session = Session::memory().await.unwrap();
    let snapshot = session.snapshot();
    assert_eq!(snapshot.revision(), Revision::ZERO);
    assert_eq!(snapshot.pages().len(), 0);
}

#[tokio::test]
async fn components_on_new_entities_do_not_observe_missing_base_state() {
    let mut session = Session::memory().await.unwrap();
    let mut entity = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let id = edit.add_entity(page, At::End)?;
            edit.set(
                id,
                &Visibility {
                    origin: Origin::User,
                    visible: true,
                    opacity: 1.0,
                },
            )?;
            entity = Some(id);
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    assert!(
        snapshot
            .component::<Visibility>(entity.unwrap())
            .unwrap()
            .is_some()
    );
}

#[tokio::test]
async fn built_in_component_schema_revisions_are_explicit() {
    fn schema<T: Component>() -> u16 {
        <T as revision::Revisioned>::revision()
    }

    assert_eq!(
        [
            schema::<Project>(),
            schema::<RasterLayer>(),
            schema::<Geometry>(),
            schema::<Visibility>(),
            schema::<SourceText>(),
            schema::<Translation>(),
            schema::<TextRole>(),
            schema::<OcrAnalysis>(),
            schema::<Group>(),
            schema::<TextGroup>(),
            schema::<Typography>(),
            schema::<Region>(),
            schema::<DetectionAnalysis>(),
            schema::<EntityOrigin>(),
            schema::<TextContent>(),
            schema::<crate::components::Assets>(),
            schema::<Relation>(),
            schema::<Speaker>(),
            schema::<BalloonClassification>(),
            schema::<Glossary>(),
        ],
        [1; 20]
    );
    assert_eq!(schema::<Page>(), 2);
    assert_eq!(schema::<TextLayout>(), 2);
    assert_eq!(<GlossaryEntry as revision::Revisioned>::revision(), 3);
    assert_eq!(<CharacterProfile as revision::Revisioned>::revision(), 2);
}

#[tokio::test]
async fn built_in_component_kinds_express_domain_ownership() {
    assert_eq!(
        [
            Project::KIND,
            Page::KIND,
            EntityOrigin::KIND,
            Relation::KIND,
            Geometry::KIND,
            Visibility::KIND,
            RasterLayer::KIND,
            TextLayout::KIND,
            TextContent::KIND,
            SourceText::KIND,
            Translation::KIND,
            TextRole::KIND,
            Group::KIND,
            TextGroup::KIND,
            Typography::KIND,
            Region::KIND,
            DetectionAnalysis::KIND,
            OcrAnalysis::KIND,
            crate::components::Assets::KIND,
            Speaker::KIND,
            BalloonClassification::KIND,
        ],
        [
            "dev.koharu.project",
            "dev.koharu.page",
            "dev.koharu.entity.origin",
            "dev.koharu.relation",
            "dev.koharu.geometry",
            "dev.koharu.visibility",
            "dev.koharu.layer.raster",
            "dev.koharu.layer.text",
            "dev.koharu.text.content",
            "dev.koharu.text.source",
            "dev.koharu.text.translation",
            "dev.koharu.text.role",
            "dev.koharu.group",
            "dev.koharu.text.group",
            "dev.koharu.text.typography",
            "dev.koharu.analysis.region",
            "dev.koharu.analysis.detection",
            "dev.koharu.analysis.ocr",
            "dev.koharu.assets",
            "dev.koharu.text.speaker",
            "dev.koharu.analysis.balloon-classification",
        ]
    );
}

#[tokio::test]
async fn revision_one_project_components_upgrade_when_their_schema_evolves() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("evolving.khrproj");
    let entity;
    {
        let mut session = Session::create(&path).await.unwrap();
        let mut created = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(page(), At::End)?;
                let id = edit.add_entity(page, At::End)?;
                edit.set(
                    id,
                    &EvolvingComponentV1 {
                        title: "revision one".to_owned(),
                        legacy_count: 7,
                    },
                )?;
                created = Some(id);
                Ok(())
            })
            .unwrap();
        session.commit(patch).await.unwrap();
        entity = created.unwrap();
    }

    let mut session = Session::open(&path).await.unwrap();
    let upgraded = session
        .snapshot()
        .component::<EvolvingComponentV2>(entity)
        .unwrap()
        .unwrap();
    assert_eq!(
        upgraded,
        EvolvingComponentV2 {
            title: "revision one".to_owned(),
            count: 7,
            label: "migrated".to_owned(),
        }
    );

    let patch = session
        .snapshot()
        .patch(|edit| edit.set(entity, &upgraded))
        .unwrap();
    session.commit(patch).await.unwrap();
    drop(session);

    let reopened = Session::open(&path).await.unwrap();
    assert_eq!(
        reopened
            .snapshot()
            .component::<EvolvingComponentV2>(entity)
            .unwrap(),
        Some(upgraded)
    );
}

#[tokio::test]
async fn revision_one_text_layout_opens_without_an_angle() {
    #[revisioned(revision = 1)]
    #[derive(Clone)]
    struct TextLayoutV1 {
        origin: Origin,
        kind: TextLayoutKind,
    }
    impl Component for TextLayoutV1 {
        const KIND: &'static str = TextLayout::KIND;
    }

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("old-text-layout.khrproj");
    let layer;
    {
        let mut session = Session::create(&path).await.unwrap();
        let mut edit = session.snapshot().edit();
        let page = edit.add_page(page(), At::End).unwrap();
        let content = edit.add_text_content(page, At::End).unwrap();
        let group = edit.ensure_text_group(page).unwrap();
        layer = edit.add_entity(group, At::End).unwrap();
        edit.set(
            layer,
            &TextLayoutV1 {
                origin: Origin::User,
                kind: TextLayoutKind::Paragraph,
            },
        )
        .unwrap();
        edit.relate::<Presents>(layer, content).unwrap();
        session.commit(edit.finish().unwrap()).await.unwrap();
    }

    let mut session = Session::open(&path).await.unwrap();
    let mut layout = session
        .snapshot()
        .component::<TextLayout>(layer)
        .unwrap()
        .unwrap();
    assert_eq!(layout.origin, Origin::User);
    assert_eq!(layout.kind, TextLayoutKind::Paragraph);
    assert_eq!(layout.angle_degrees, None);
    layout.angle_degrees = Some(27.0);
    let patch = session
        .snapshot()
        .patch(|edit| edit.set(layer, &layout))
        .unwrap();
    session.commit(patch).await.unwrap();
    drop(session);

    let reopened = Session::open(&path).await.unwrap();
    assert_eq!(
        reopened.snapshot().component::<TextLayout>(layer).unwrap(),
        Some(layout)
    );
}

#[tokio::test]
async fn stale_disjoint_patches_can_rebase_without_hiding_conflicts() {
    let mut session = Session::memory().await.unwrap();
    let mut entities = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            entities = Some((
                edit.add_entity(page, At::End)?,
                edit.add_entity(page, At::End)?,
            ));
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let (left, right) = entities.unwrap();
    let left_patch = base
        .patch(|edit| edit.set(left, &Geometry::rectangle(0.0, 0.0, 10.0, 10.0)))
        .unwrap();
    let right_patch = base
        .patch(|edit| edit.set(right, &Geometry::rectangle(1.0, 1.0, 10.0, 10.0)))
        .unwrap();
    let conflicting = base
        .patch(|edit| edit.set(left, &Geometry::rectangle(2.0, 2.0, 10.0, 10.0)))
        .unwrap();

    let current = session.commit(left_patch).await.unwrap().snapshot;
    let current = session
        .commit(right_patch.rebase_on(&current).unwrap())
        .await
        .unwrap()
        .snapshot;
    assert!(conflicting.rebase_on(&current).is_err());
}

#[tokio::test]
async fn hierarchy_insertion_rebases_across_component_edits_but_not_sibling_changes() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(page, At::End)?;
            ids = Some((page, content));
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let (page, content) = ids.unwrap();

    let insert = base
        .patch(|edit| {
            edit.add_entity(page, At::Start)?;
            Ok(())
        })
        .unwrap();
    let component_edit = base
        .patch(|edit| edit.set(content, &source("translated")))
        .unwrap();
    let current = base.preview([&component_edit]).unwrap();
    assert!(insert.rebase_on(&current).is_ok());

    let sibling_insert = base
        .patch(|edit| {
            edit.add_entity(page, At::End)?;
            Ok(())
        })
        .unwrap();
    let current = base.preview([&sibling_insert]).unwrap();
    assert!(insert.rebase_on(&current).is_err());
}

#[tokio::test]
async fn observed_page_subtree_guards_pipeline_rebase_inputs() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let left_page = edit.add_page(PageDraft::new("left", 100.0, 100.0), At::End)?;
            let left_entity = edit.add_text_content(left_page, At::End)?;
            let right_page = edit.add_page(PageDraft::new("right", 100.0, 100.0), At::End)?;
            ids = Some((left_page, left_entity, right_page));
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let (left_page, left_entity, right_page) = ids.unwrap();

    let observed = base
        .patch(|edit| {
            edit.observe_subtree(left_page)?;
            edit.set(left_entity, &source("derived"))
        })
        .unwrap();
    let unrelated = base
        .patch(|edit| edit.set_page(right_page, PageDraft::new("right changed", 100.0, 100.0)))
        .unwrap();
    let current = session.commit(unrelated).await.unwrap().snapshot;
    assert!(observed.rebase_on(&current).is_ok());

    let observed = current
        .patch(|edit| {
            edit.observe_subtree(left_page)?;
            edit.set(left_entity, &source("stale"))
        })
        .unwrap();
    let changed_input = current
        .patch(|edit| edit.set_page(left_page, PageDraft::new("left changed", 100.0, 100.0)))
        .unwrap();
    let current = session.commit(changed_input).await.unwrap().snapshot;
    assert!(observed.rebase_on(&current).is_err());
}

#[tokio::test]
async fn project_and_relation_metadata_use_typed_components() {
    let mut session = Session::memory().await.unwrap();
    let settings = Project {
        source_locale: Some(LanguageTag::new("ja").unwrap()),
        target_locales: vec![LanguageTag::new("en").unwrap()],
    };
    let kind = RelationKind::new("dev.koharu.test.link").unwrap();
    let mut relation = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            edit.set_project(&settings)?;
            let page = edit.add_page(page(), At::End)?;
            let left = edit.add_entity(page, At::End)?;
            let right = edit.add_entity(page, At::End)?;
            let id = edit.add_relation(kind, left, right)?;
            edit.set_relation(
                id,
                &Visibility {
                    origin: Origin::User,
                    visible: true,
                    opacity: 0.5,
                },
            )?;
            relation = Some(id);
            Ok(())
        })
        .unwrap();
    let commit = session.commit(patch).await.unwrap();
    let snapshot = commit.snapshot;
    assert!(
        commit.changes.components.iter().any(|change| {
            change.owner == ComponentOwner::Project && change.kind == Project::KIND
        })
    );
    assert!(commit.changes.components.iter().any(|change| {
        change.owner == ComponentOwner::Relation(relation.unwrap())
            && change.kind == Visibility::KIND
    }));
    assert_eq!(
        snapshot.project_component::<Project>().unwrap(),
        Some(settings)
    );
    assert_eq!(
        snapshot
            .relation(relation.unwrap())
            .unwrap()
            .component::<Visibility>()
            .unwrap()
            .unwrap()
            .opacity,
        0.5
    );
}

#[tokio::test]
async fn producer_reruns_respect_component_ownership() {
    let mut session = Session::memory().await.unwrap();
    let mut entities = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let user = edit.add_text_content(page, At::End)?;
            edit.set(user, &source("user"))?;
            let generated = edit.add_text_content(page, At::End)?;
            entities = Some((user, generated));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let (user, generated) = entities.unwrap();
    let producer = ProducerId::new("dev.koharu.pipeline.ocr").unwrap();
    let generation = Generation::new(producer.clone());
    let mut edit = snapshot.edit_as(generation.clone());
    assert!(matches!(
        edit.set(user, &source("overwrite")),
        Err(Error::Authorship(_))
    ));
    edit.set(generated, &source("generated")).unwrap();
    let snapshot = session
        .commit(edit.finish().unwrap())
        .await
        .unwrap()
        .snapshot;

    let mut rerun = snapshot.edit_as(generation);
    rerun.set(generated, &source("generated again")).unwrap();
    let snapshot = session
        .commit(rerun.finish().unwrap())
        .await
        .unwrap()
        .snapshot;
    let other = Generation::new(ProducerId::new("dev.koharu.pipeline.other-ocr").unwrap());
    let mut other_edit = snapshot.edit_as(other);
    assert!(matches!(
        other_edit.set(generated, &source("wrong owner")),
        Err(Error::Authorship(_))
    ));
}

#[tokio::test]
async fn pipeline_removal_respects_entity_lifecycle_owner() {
    let mut session = Session::memory().await.unwrap();
    let mut page_id = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            page_id = Some(edit.add_page(page(), At::End)?);
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let page = page_id.unwrap();
    let owner = Generation::new(ProducerId::new("dev.koharu.pipeline.detector").unwrap());
    let mut edit = snapshot.edit_as(owner.clone());
    let generated = edit.add_entity(page, At::End).unwrap();
    let snapshot = session
        .commit(edit.finish().unwrap())
        .await
        .unwrap()
        .snapshot;

    let mut other = snapshot.edit_as(Generation::new(
        ProducerId::new("dev.koharu.pipeline.other-detector").unwrap(),
    ));
    assert!(matches!(
        other.remove_entity(generated, RemovePolicy::Cascade),
        Err(Error::Authorship(_))
    ));

    let mut owner_edit = snapshot.edit_as(owner);
    owner_edit
        .remove_entity(generated, RemovePolicy::Cascade)
        .unwrap();
    let snapshot = session
        .commit(owner_edit.finish().unwrap())
        .await
        .unwrap()
        .snapshot;
    assert!(snapshot.entity(generated).is_err());
}

#[tokio::test]
async fn typed_page_entity_and_components_round_trip() {
    let mut session = Session::memory().await.unwrap();
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let text = edit.add_text_content(page, At::End)?;
            edit.set(text, &source("こんにちは"))?;
            Ok(())
        })
        .unwrap();
    let commit = session.commit(patch).await.unwrap();
    let page = commit.snapshot.pages().next().unwrap();
    assert_eq!(page.page().unwrap().label, "page");
    let text = commit.snapshot.children(page.id()).unwrap().next().unwrap();
    assert_eq!(
        commit
            .snapshot
            .component::<SourceText>(text)
            .unwrap()
            .unwrap()
            .text
            .value,
        "こんにちは"
    );
}

#[tokio::test]
async fn text_analysis_content_and_presentation_have_distinct_ownership() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let region = edit.add_analysis_region::<TextRegion>(
                page,
                At::End,
                &Geometry::rectangle(10.0, 20.0, 100.0, 40.0),
                None,
            )?;
            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &source("source"))?;
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
            edit.relate::<RecognizedFrom>(content, region)?;
            edit.relate::<FitsTo>(layer, region)?;
            ids = Some((content, layer));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let (content, layer) = ids.unwrap();
    assert_eq!(
        snapshot
            .relation_from::<Presents>(layer)
            .unwrap()
            .unwrap()
            .value()
            .target,
        content
    );
    assert!(matches!(
        snapshot.patch(|edit| edit.set(content, &Geometry::rectangle(0.0, 0.0, 10.0, 10.0))),
        Err(Error::Invalid(_))
    ));
}

#[tokio::test]
async fn text_group_owns_the_canonical_text_order() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let first_content = edit.add_text_content(page, At::End)?;
            let first = edit.add_text_layer(
                page,
                At::End,
                first_content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            let second_content = edit.add_text_content(page, At::End)?;
            let second = edit.add_text_layer(
                page,
                At::End,
                second_content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?;
            ids = Some((page, first, second));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let (page, first, second) = ids.unwrap();
    let group = snapshot.page(page).unwrap().text_group().unwrap().unwrap();
    assert_eq!(
        group
            .text_layers()
            .unwrap()
            .map(TextLayerRef::id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    let patch = snapshot
        .patch(|edit| edit.move_entity(second, Some(group.id()), At::Start))
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    assert_eq!(
        snapshot
            .page(page)
            .unwrap()
            .text_group()
            .unwrap()
            .unwrap()
            .text_layers()
            .unwrap()
            .map(TextLayerRef::id)
            .collect::<Vec<_>>(),
        vec![second, first]
    );
}

#[tokio::test]
async fn typed_relations_enforce_endpoints_and_functional_cardinality() {
    let session = Session::memory().await.unwrap();
    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let region = edit.add_analysis_region::<TextRegion>(
            page,
            At::End,
            &Geometry::rectangle(0.0, 0.0, 10.0, 10.0),
            None,
        )?;
        let layer = edit.add_entity(page, At::End)?;
        edit.set(
            layer,
            &TextLayout {
                origin: Origin::User,
                kind: TextLayoutKind::Paragraph,
                angle_degrees: None,
            },
        )?;
        edit.relate::<Presents>(layer, region)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Invalid(_))));

    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let first = edit.add_text_content(page, At::End)?;
        let second = edit.add_text_content(page, At::End)?;
        let layer = edit.add_text_layer(
            page,
            At::End,
            first,
            &TextLayout {
                origin: Origin::User,
                kind: TextLayoutKind::Paragraph,
                angle_degrees: None,
            },
        )?;
        edit.relate::<Presents>(layer, second)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Invalid(_))));

    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let bubble = edit.add_analysis_region::<BubbleRegion>(
            page,
            At::End,
            &Geometry::rectangle(0.0, 0.0, 20.0, 20.0),
            None,
        )?;
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
        edit.relate::<FitsTo>(layer, bubble)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Invalid(_))));

    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let text_region = edit.add_analysis_region::<TextRegion>(
            page,
            At::End,
            &Geometry::rectangle(0.0, 0.0, 10.0, 10.0),
            None,
        )?;
        let bubble = edit.add_analysis_region::<BubbleRegion>(
            page,
            At::End,
            &Geometry::rectangle(0.0, 0.0, 20.0, 20.0),
            None,
        )?;
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
        edit.relate::<FitsTo>(layer, text_region)?;
        edit.relate::<FlowsIn>(layer, bubble)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Invalid(_))));

    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let text_region = edit.add_analysis_region::<TextRegion>(
            page,
            At::End,
            &Geometry::rectangle(0.0, 0.0, 10.0, 10.0),
            None,
        )?;
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
        edit.relate::<FlowsIn>(layer, text_region)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[tokio::test]
async fn component_changes_cannot_invalidate_incident_typed_relations() {
    let mut session = Session::memory().await.unwrap();
    let mut layer = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(page, At::End)?;
            layer = Some(edit.add_text_layer(
                page,
                At::End,
                content,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Paragraph,
                    angle_degrees: None,
                },
            )?);
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let result = snapshot.patch(|edit| edit.remove::<TextLayout>(layer.unwrap()));
    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[tokio::test]
async fn hierarchy_move_and_promote_are_ordered() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let group = edit.add_entity(page, At::End)?;
            let child = edit.add_text_content(group, At::End)?;
            let sibling = edit.add_entity(page, At::End)?;
            ids = Some((page, group, child, sibling));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let (page, group, child, sibling) = ids.unwrap();
    let patch = snapshot
        .patch(|edit| {
            edit.move_entity(sibling, Some(group), At::Start)?;
            edit.remove_entity(group, RemovePolicy::PromoteChildren)
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    assert_eq!(
        snapshot.children(page).unwrap().collect::<Vec<_>>(),
        vec![sibling, child]
    );
    assert_eq!(snapshot.parent(child).unwrap(), Some(page));
    assert!(matches!(
        snapshot.entity(group),
        Err(Error::EntityNotFound(_))
    ));
}

#[tokio::test]
async fn cross_page_subtree_move_preserves_identity_components_relations_and_undo() {
    let mut session = Session::memory().await.unwrap();
    let kind = RelationKind::new("dev.koharu.test.cross-page").unwrap();
    let mut ids = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let source_page = edit.add_page(page(), At::End)?;
            let target_page = edit.add_page(page(), At::End)?;
            let group = edit.add_entity(source_page, At::End)?;
            let child = edit.add_text_content(group, At::End)?;
            edit.set(child, &source("stable"))?;
            let relation = edit.add_relation(kind.clone(), child, target_page)?;
            ids = Some((source_page, target_page, group, child, relation));
            Ok(())
        })
        .unwrap();
    session.commit(create).await.unwrap();
    let (source_page, target_page, group, child, relation) = ids.unwrap();

    let moved = session
        .snapshot()
        .patch(|edit| edit.move_entity(group, Some(target_page), At::End))
        .unwrap();
    let commit = session.commit(moved).await.unwrap();
    assert!(
        commit
            .snapshot
            .children(source_page)
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(commit.snapshot.parent(group).unwrap(), Some(target_page));
    assert_eq!(commit.snapshot.parent(child).unwrap(), Some(group));
    assert_eq!(
        commit
            .snapshot
            .component::<SourceText>(child)
            .unwrap()
            .unwrap()
            .text
            .value,
        "stable"
    );
    assert_eq!(
        commit.snapshot.relation(relation).unwrap().value().source,
        child
    );

    let undone = session.undo(commit.revision).await.unwrap().snapshot;
    assert_eq!(undone.parent(group).unwrap(), Some(source_page));
    assert_eq!(undone.parent(child).unwrap(), Some(group));
    assert_eq!(undone.relation(relation).unwrap().value().source, child);
}

#[tokio::test]
async fn relations_are_records_with_typed_adjacency() {
    let mut session = Session::memory().await.unwrap();
    let kind = RelationKind::new("dev.koharu.test.reading-order").unwrap();
    let mut ids = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let first = edit.add_entity(page, At::End)?;
            let second = edit.add_entity(page, At::End)?;
            let relation = edit.add_relation(kind.clone(), first, second)?;
            ids = Some((first, second, relation));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let (first, second, relation) = ids.unwrap();
    assert_eq!(
        snapshot
            .relations_from(first, Some(&kind))
            .next()
            .unwrap()
            .id(),
        relation
    );
    assert_eq!(
        snapshot
            .relations_to(second, None)
            .next()
            .unwrap()
            .value()
            .source,
        first
    );
    assert!(matches!(
        snapshot.patch(|edit| edit.remove_entity(first, RemovePolicy::RejectNonEmpty)),
        Err(Error::IncidentRelations(id)) if id == first
    ));
    let patch = snapshot
        .patch(|edit| edit.remove_entity(first, RemovePolicy::Cascade))
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    assert!(snapshot.relation(relation).is_err());
}

#[tokio::test]
async fn translation_requires_source_text() {
    let session = Session::memory().await.unwrap();
    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let entity = edit.add_entity(page, At::End)?;
        edit.set(
            entity,
            &Translation {
                text: Authored::user("hello".to_owned()),
                language: Some(LanguageTag::new("en").unwrap()),
            },
        )
    });
    assert!(matches!(result, Err(Error::Invalid(_))));
}

#[tokio::test]
async fn text_components_have_one_value_per_entity() {
    let session = Session::memory().await.unwrap();
    let result = session.snapshot().patch(|edit| {
        let page = edit.add_page(page(), At::End)?;
        let entity = edit.add_text_content(page, At::End)?;
        edit.set(entity, &source("source"))?;
        edit.set(
            entity,
            &Translation {
                text: Authored::user("translation".to_owned()),
                language: Some(LanguageTag::new("en").unwrap()),
            },
        )
    });
    assert!(result.is_ok());
}

#[tokio::test]
async fn independent_pipeline_components_rebase() {
    let mut session = Session::memory().await.unwrap();
    let mut entities = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &source("source"))?;
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
            entities = Some((content, layer));
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let (content, layer) = entities.unwrap();
    let translation = base
        .patch(|edit| {
            edit.set(
                content,
                &Translation {
                    text: Authored::user("translation".to_owned()),
                    language: Some(LanguageTag::new("en").unwrap()),
                },
            )
        })
        .unwrap();
    let typography = base
        .patch(|edit| {
            edit.set(
                layer,
                &Typography {
                    origin: Origin::User,
                    preferred_font: Some("Inter".to_owned()),
                    font_weight: Some(500),
                    font_style: Some(FontStyle::Italic),
                    size: Some(24.0),
                    auto_fit: false,
                    color: Some([1, 2, 3, 255]),
                    stroke_color: None,
                    stroke_width: None,
                    alignment: Some(TextAlignment::Center),
                    writing_mode: None,
                    extensions: BTreeMap::new(),
                },
            )
        })
        .unwrap();
    let current = session.commit(translation).await.unwrap().snapshot;
    let typography = typography.rebase_on(&current).unwrap();
    let snapshot = session.commit(typography).await.unwrap().snapshot;
    assert!(
        snapshot
            .component::<Translation>(content)
            .unwrap()
            .is_some()
    );
    assert!(snapshot.component::<Typography>(layer).unwrap().is_some());
}

#[tokio::test]
async fn descendant_scene_patch_rebases_after_ancestor_commit() {
    let mut session = Session::memory().await.unwrap();
    let base = session.snapshot();
    let mut edit = base.edit();
    let page = edit.add_page(page(), At::End).unwrap();
    let ancestor = edit.finish().unwrap();
    let preview = base.preview([&ancestor]).unwrap();
    let descendant = preview
        .patch(|edit| {
            let entity = edit.add_text_content(page, At::End)?;
            edit.set(entity, &source("late"))
        })
        .unwrap();
    assert!(base.preview([&ancestor, &descendant]).is_ok());
    let current = session.commit(ancestor).await.unwrap().snapshot;
    let descendant = descendant.rebase_on(&current).unwrap();
    let snapshot = session.commit(descendant).await.unwrap().snapshot;
    assert_eq!(snapshot.children(page).unwrap().len(), 1);
}

#[tokio::test]
async fn assets_attach_bytes_without_decoding() {
    let mut session = Session::memory().await.unwrap();
    let role = AssetRole::new("source").unwrap();
    let mut entity = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            entity = Some(page);
            edit.set_asset(
                page,
                &role,
                AssetInput::new(
                    Arc::<[u8]>::from(&b"encoded image"[..]),
                    "image/test",
                    AssetMetadata {
                        width: Some(10),
                        height: Some(20),
                        attributes: BTreeMap::new(),
                    },
                ),
            )
        })
        .unwrap();
    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let asset = snapshot.asset(entity.unwrap(), &role).unwrap().unwrap();
    assert_eq!(
        snapshot.read_blob(asset.blob).await.unwrap().as_ref(),
        b"encoded image"
    );
}

#[tokio::test]
async fn repeated_asset_writes_rebase_over_an_unrelated_page_edit() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let first_page = edit.add_page(page(), At::End)?;
            let second_page = edit.add_page(page(), At::End)?;
            ids = Some((first_page, second_page));
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let (first_page, second_page) = ids.unwrap();

    let assets = base
        .patch(|edit| {
            for role in ["text-mask", "bubble-mask"] {
                edit.set_asset(
                    second_page,
                    &AssetRole::new(role)?,
                    AssetInput::new(
                        Arc::<[u8]>::from(role.as_bytes()),
                        "image/test",
                        AssetMetadata {
                            width: None,
                            height: None,
                            attributes: BTreeMap::new(),
                        },
                    ),
                )?;
            }
            Ok(())
        })
        .unwrap();
    let unrelated = base
        .patch(|edit| {
            edit.set_asset(
                first_page,
                &AssetRole::new("source")?,
                AssetInput::new(
                    Arc::<[u8]>::from(&b"source"[..]),
                    "image/test",
                    AssetMetadata {
                        width: None,
                        height: None,
                        attributes: BTreeMap::new(),
                    },
                ),
            )
        })
        .unwrap();
    let current = session.commit(unrelated).await.unwrap().snapshot;

    let assets = assets.rebase_on(&current).unwrap();
    let snapshot = session.commit(assets).await.unwrap().snapshot;
    for role in ["text-mask", "bubble-mask"] {
        assert!(
            snapshot
                .asset(second_page, &AssetRole::new(role).unwrap())
                .unwrap()
                .is_some()
        );
    }
}

#[tokio::test]
async fn disk_scene_reopens_and_validates() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("scene.khrproj");
    {
        let mut session = Session::create(&path).await.unwrap();
        let patch = session
            .snapshot()
            .patch(|edit| edit.add_page(page(), At::End).map(|_| ()))
            .unwrap();
        session.commit(patch).await.unwrap();
    }
    let session = Session::open(&path).await.unwrap();
    assert_eq!(session.snapshot().pages().len(), 1);
}

#[tokio::test]
async fn reopen_updates_page_local_component_queries() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("refresh.khrproj");
    let mut writer = Session::create(&path).await.unwrap();
    let mut entity = None;
    let create = writer
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            entity = Some(edit.add_text_content(page, At::End)?);
            Ok(())
        })
        .unwrap();
    writer.commit(create).await.unwrap();
    let entity = entity.unwrap();

    let update = writer
        .snapshot()
        .patch(|edit| edit.set(entity, &source("refreshed")))
        .unwrap();
    writer.commit(update).await.unwrap();
    drop(writer);

    let reader = Session::open(&path).await.unwrap();
    let snapshot = reader.snapshot();
    assert_eq!(
        snapshot
            .component::<SourceText>(entity)
            .unwrap()
            .unwrap()
            .text
            .value,
        "refreshed"
    );
}

#[tokio::test]
async fn pipeline_component_removal_and_relation_lifecycle_respect_ownership() {
    let mut session = Session::memory().await.unwrap();
    let role = AssetRole::new("source").unwrap();
    let relation_kind = RelationKind::new("dev.koharu.test.association").unwrap();
    let mut ids = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let left = edit.add_text_content(page, At::End)?;
            let right = edit.add_entity(page, At::End)?;
            let analysis = edit.add_entity(page, At::End)?;
            edit.set(left, &source("user text"))?;
            edit.set(analysis, &Geometry::rectangle(0.0, 0.0, 10.0, 10.0))?;
            edit.set(
                analysis,
                &Region {
                    origin: Origin::User,
                    kind: TextRegion::kind(),
                    label: None,
                },
            )?;
            edit.set_asset(
                left,
                &role,
                AssetInput::new(
                    Arc::<[u8]>::from(&b"user asset"[..]),
                    "image/test",
                    AssetMetadata {
                        width: Some(1),
                        height: Some(1),
                        attributes: BTreeMap::new(),
                    },
                ),
            )?;
            let relation = edit.add_relation(relation_kind.clone(), left, right)?;
            ids = Some((left, analysis, relation));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(create).await.unwrap().snapshot;
    let (entity, analysis_entity, relation) = ids.unwrap();
    let generation = Generation::new(ProducerId::new("dev.koharu.pipeline.detector").unwrap());
    let mut pipeline = snapshot.edit_as(generation.clone());
    assert!(matches!(
        pipeline.remove::<SourceText>(entity),
        Err(Error::Authorship(_))
    ));
    assert!(matches!(
        pipeline.remove_asset(entity, &role),
        Err(Error::Authorship(_))
    ));
    assert!(matches!(
        pipeline.remove_relation(relation),
        Err(Error::Authorship(_))
    ));

    let analysis = DetectionAnalysis {
        origin: Origin::User,
        labels: vec![DetectionLabel {
            kind: RegionKind::new("dev.koharu.region.text").unwrap(),
            confidence: 0.9,
        }],
    };
    pipeline
        .set(analysis_entity, &analysis)
        .expect("new pipeline analysis is stamped by the edit context");
    let snapshot = session
        .commit(pipeline.finish().unwrap())
        .await
        .unwrap()
        .snapshot;
    let stored = snapshot
        .component::<DetectionAnalysis>(analysis_entity)
        .unwrap()
        .unwrap();
    assert!(matches!(stored.origin, Origin::Generated(_)));

    let mut other = snapshot.edit_as(Generation::new(
        ProducerId::new("dev.koharu.pipeline.other-detector").unwrap(),
    ));
    assert!(matches!(
        other.set(analysis_entity, &analysis),
        Err(Error::Authorship(_))
    ));
    assert!(matches!(
        other.remove::<DetectionAnalysis>(analysis_entity),
        Err(Error::Authorship(_))
    ));
}

#[tokio::test]
async fn pipeline_queries_and_analysis_components_are_semantic_and_ordered() {
    let mut session = Session::memory().await.unwrap();
    let mut ids = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let first = edit.add_text_content(page, At::End)?;
            let nested = edit.add_text_content(first, At::End)?;
            let second = edit.add_entity(page, At::End)?;
            edit.set(first, &source("first"))?;
            edit.set(nested, &source("nested"))?;
            edit.set(second, &Geometry::rectangle(0.0, 0.0, 10.0, 10.0))?;
            edit.set(
                second,
                &Region {
                    origin: Origin::User,
                    kind: TextRegion::kind(),
                    label: None,
                },
            )?;
            ids = Some((page, first, nested, second));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(create).await.unwrap().snapshot;
    let (page, first, nested, second) = ids.unwrap();
    assert_eq!(
        snapshot.entities().map(EntityRef::id).collect::<Vec<_>>(),
        vec![page, first, nested, second]
    );
    assert_eq!(
        snapshot
            .entities_with::<SourceText>()
            .unwrap()
            .map(EntityRef::id)
            .collect::<Vec<_>>(),
        vec![first, nested]
    );
    assert_eq!(
        snapshot
            .subtree(first)
            .unwrap()
            .map(EntityRef::id)
            .collect::<Vec<_>>(),
        vec![first, nested]
    );
    assert_eq!(
        snapshot
            .descendants(first)
            .unwrap()
            .map(EntityRef::id)
            .collect::<Vec<_>>(),
        vec![nested]
    );

    let generation = Generation::new(ProducerId::new("dev.koharu.pipeline.ocr").unwrap());
    let mut edit = snapshot.edit_as(generation);
    edit.set(
        second,
        &OcrAnalysis {
            origin: Origin::User,
            direction: TextDirection::Vertical,
            confidence: Some(0.95),
            line_boundaries: vec![[
                Point { x: 0.0, y: 0.0 },
                Point { x: 1.0, y: 0.0 },
                Point { x: 1.0, y: 1.0 },
                Point { x: 0.0, y: 1.0 },
            ]],
        },
    )
    .unwrap();
    let snapshot = session
        .commit(edit.finish().unwrap())
        .await
        .unwrap()
        .snapshot;
    assert!(matches!(
        snapshot
            .component::<OcrAnalysis>(second)
            .unwrap()
            .unwrap()
            .origin,
        Origin::Generated(_)
    ));
}

#[tokio::test]
async fn scene_patch_exposes_stable_identity_and_base() {
    let mut session = Session::memory().await.unwrap();
    let mut entity = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            entity = Some(edit.add_text_content(page, At::End)?);
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let entity = entity.unwrap();
    let patch = base
        .patch(|edit| edit.set(entity, &source("identity")))
        .unwrap();
    assert_eq!(patch.project_id(), base.project_id());
    assert_eq!(patch.base_revision(), base.revision());
    assert_eq!(patch.fingerprint(), patch.clone().fingerprint());
    assert_ne!(
        patch.fingerprint(),
        patch
            .clone()
            .with_label("different commit label")
            .fingerprint()
    );
}

#[tokio::test]
async fn component_snapshots_clone_only_the_touched_page() {
    let mut session = Session::memory().await.unwrap();
    let mut entity = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            entity = Some(edit.add_text_content(page, At::End)?);
            Ok(())
        })
        .unwrap();
    let base = session.commit(create).await.unwrap().snapshot;
    let entity = entity.unwrap();
    let page = base.state.page_for(entity).unwrap();
    let base_page = base.state.pages[&page].clone();

    let patch = base
        .patch(|edit| edit.set(entity, &source("cached")))
        .unwrap();
    let preview = base.preview([&patch]).unwrap();
    assert!(!Arc::ptr_eq(&preview.state.pages[&page], &base_page));
    assert_eq!(preview.entities_with::<SourceText>().unwrap().len(), 1);
    let committed = session.commit(patch).await.unwrap().snapshot;
    assert!(!Arc::ptr_eq(&committed.state.pages[&page], &base_page));
    let first = session.snapshot();
    let second = session.snapshot();
    assert!(Arc::ptr_eq(&first.state, &second.state));
}

#[tokio::test]
async fn user_promotion_protects_generated_entity_and_relation_lifecycle() {
    let mut session = Session::memory().await.unwrap();
    let mut endpoints = None;
    let create = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let left = edit.add_entity(page, At::End)?;
            let right = edit.add_entity(page, At::End)?;
            endpoints = Some((left, right));
            Ok(())
        })
        .unwrap();
    let snapshot = session.commit(create).await.unwrap().snapshot;
    let (left, right) = endpoints.unwrap();
    let generation = Generation::new(ProducerId::new("dev.koharu.pipeline.detector").unwrap());
    let mut generated = snapshot.edit_as(generation.clone());
    let entity = generated.add_entity(left, At::End).unwrap();
    let relation = generated
        .add_relation(
            RelationKind::new("dev.koharu.test.generated-link").unwrap(),
            entity,
            right,
        )
        .unwrap();
    let snapshot = session
        .commit(generated.finish().unwrap())
        .await
        .unwrap()
        .snapshot;

    let promoted = snapshot
        .patch(|edit| {
            edit.promote_entity_to_user(entity)?;
            edit.promote_relation_to_user(relation)
        })
        .unwrap();
    let snapshot = session.commit(promoted).await.unwrap().snapshot;
    let mut rerun = snapshot.edit_as(generation);
    assert!(matches!(
        rerun.remove_relation(relation),
        Err(Error::Authorship(_))
    ));
    assert!(matches!(
        rerun.remove_entity(entity, RemovePolicy::Cascade),
        Err(Error::Authorship(_))
    ));
}

#[tokio::test]
async fn textbox_edits_preserve_inpainted_raster_layer_assets() {
    let mut session = Session::memory().await.unwrap();
    let source_role = AssetRole::new("source").unwrap();
    let mut page_id = None;
    let mut cleanup_layer_id = None;
    let mut text_layer_id = None;
    let mut text_content_id = None;

    let init = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let cleanup = edit.add_entity(page, At::End)?;
            edit.set(
                cleanup,
                &RasterLayer {
                    origin: Origin::User,
                    name: "Cleanup".to_owned(),
                    kind: RasterLayerKind::Cleanup,
                },
            )?;
            edit.set_asset(
                cleanup,
                &source_role,
                AssetInput::new(
                    Arc::<[u8]>::from(&b"inpainted clean plate bytes"[..]),
                    "image/png",
                    AssetMetadata {
                        width: Some(100),
                        height: Some(100),
                        attributes: BTreeMap::new(),
                    },
                ),
            )?;

            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &source("Hello world"))?;
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

            page_id = Some(page);
            cleanup_layer_id = Some(cleanup);
            text_layer_id = Some(layer);
            text_content_id = Some(content);
            Ok(())
        })
        .unwrap();

    let snapshot = session.commit(init).await.unwrap().snapshot;
    let cleanup = cleanup_layer_id.unwrap();
    let layer = text_layer_id.unwrap();
    let content = text_content_id.unwrap();

    let initial_asset = snapshot.asset(cleanup, &source_role).unwrap().unwrap();
    let initial_blob = initial_asset.blob;

    let text_edit = snapshot
        .patch(|edit| {
            edit.set(
                layer,
                &TextLayout {
                    origin: Origin::User,
                    kind: TextLayoutKind::Point,
                    angle_degrees: Some(15.0),
                },
            )?;
            edit.set(
                content,
                &Translation {
                    text: Authored::user("Translated text".to_owned()),
                    language: Some(LanguageTag::new("en").unwrap()),
                },
            )?;
            edit.set(
                layer,
                &Typography {
                    origin: Origin::User,
                    preferred_font: Some("Comic Sans".to_owned()),
                    font_weight: Some(700),
                    font_style: None,
                    size: Some(16.0),
                    auto_fit: false,
                    color: Some([0, 0, 0, 255]),
                    stroke_color: None,
                    stroke_width: None,
                    alignment: None,
                    writing_mode: None,
                    extensions: BTreeMap::new(),
                },
            )?;
            Ok(())
        })
        .unwrap();

    let after_text_edit = session.commit(text_edit).await.unwrap().snapshot;

    let cleanup_raster = after_text_edit
        .component::<RasterLayer>(cleanup)
        .unwrap()
        .expect("cleanup layer component must exist");
    assert_eq!(cleanup_raster.kind, RasterLayerKind::Cleanup);

    let preserved_asset = after_text_edit
        .asset(cleanup, &source_role)
        .unwrap()
        .expect("cleanup source asset must exist");
    assert_eq!(preserved_asset.blob, initial_blob);

    let bytes = after_text_edit.read_blob(preserved_asset.blob).await.unwrap();
    assert_eq!(bytes.as_ref(), b"inpainted clean plate bytes");
}

#[tokio::test]
async fn character_profile_and_speaker_lifecycle() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let profile = CharacterProfile {
        id: entry_id,
        gender: CharacterGender::Female,
        first_person: Some("私".to_owned()),
        second_person: Some("あなた".to_owned()),
        tone: Some("polite".to_owned()),
        custom_instructions: Some("Speak gently".to_owned()),
        honorific_policy: Some("keep".to_owned()),
    };
    let glossary = Glossary {
        enabled: true,
        source_language: Some(LanguageTag::new("ja").unwrap()),
        target_language: Some(LanguageTag::new("en").unwrap()),
        source_fingerprint: None,
        entries: vec![GlossaryEntry {
            id: entry_id,
            source: "山田".to_owned(),
            translation: Some("Yamada".to_owned()),
            kind: GlossaryKind::Person,
            enabled: true,
            confidence: Some(1.0),
            occurrence_count: 5,
            examples: vec![],
            source_origin: GlossaryValueOrigin::User,
            translation_origin: Some(GlossaryValueOrigin::User),
            present_in_last_scan: true,
            character: Some(profile.clone()),
            forbidden_variants: vec!["Yama-chan".to_owned()],
        }],
    };

    let mut text_layer = None;
    let patch = session
        .snapshot()
        .patch(|edit| {
            edit.set_project(&glossary)?;
            let page = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(page, At::End)?;
            edit.set(content, &source("こんにちは"))?;
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
            text_layer = Some(layer);
            Ok(())
        })
        .unwrap();

    let snapshot = session.commit(patch).await.unwrap().snapshot;
    let project_glossary = snapshot
        .project_component::<Glossary>()
        .unwrap()
        .expect("glossary must exist");
    assert_eq!(project_glossary.entries.len(), 1);
    assert_eq!(
        project_glossary.entries[0].character.as_ref().unwrap().gender,
        CharacterGender::Female
    );

    let layer_ref = snapshot.text_layer(text_layer.unwrap()).unwrap();
    assert_eq!(layer_ref.speaker().unwrap().and_then(|s| s.speaker), Some(entry_id));

    let page_id = snapshot.pages().next().unwrap().id();
    let producer = ProducerId::new("dev.koharu.pipeline.detection").unwrap();
    let generation = Generation::new(producer);
    let mut edit = snapshot.edit_as(generation);
    let bubble = edit.add_analysis_region::<BubbleRegion>(
        page_id,
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
    ).unwrap();
    edit.set(
        bubble,
        &BalloonClassification {
            origin: Origin::User,
            shape: BalloonShape::Shout,
            confidence: 0.95,
        },
    ).unwrap();
    let pipeline_patch = edit.finish().unwrap();

    let committed_snapshot = session.commit(pipeline_patch).await.unwrap().snapshot;
    let bubble_ref = committed_snapshot.analysis_region(bubble).unwrap();
    assert_eq!(
        bubble_ref.balloon_classification().unwrap().unwrap().shape,
        BalloonShape::Shout
    );
}


#[tokio::test]
async fn remove_entity_returns_entity_not_found_on_missing_entity() {
    let session = Session::memory().await.unwrap();
    let missing_id = EntityId::new();

    let result = session
        .snapshot()
        .patch(|edit| edit.remove_entity(missing_id, RemovePolicy::Cascade));

    match result {
        Err(Error::EntityNotFound(id)) => assert_eq!(id, missing_id),
        other => panic!("expected Error::EntityNotFound({missing_id:?}), got {other:?}"),
    }
}

#[tokio::test]
async fn remove_entity_called_twice_fails_on_second_invocation() {
    let mut session = Session::memory().await.unwrap();
    let mut entity_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let entity = edit.add_text_content(page, At::End)?;
            entity_id = Some(entity);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();
    let entity = entity_id.unwrap();

    let result = session.snapshot().patch(|edit| {
        edit.remove_entity(entity, RemovePolicy::Cascade)?;
        edit.remove_entity(entity, RemovePolicy::Cascade)?;
        Ok(())
    });

    match result {
        Err(Error::EntityNotFound(id)) => assert_eq!(id, entity),
        other => panic!("expected Error::EntityNotFound({entity:?}), got {other:?}"),
    }
}

#[tokio::test]
async fn remove_entity_if_present_safely_handles_missing_and_existing_entities() {
    let mut session = Session::memory().await.unwrap();
    let mut entity_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let entity = edit.add_text_content(page, At::End)?;
            entity_id = Some(entity);
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();
    let entity = entity_id.unwrap();
    let missing_id = EntityId::new();

    let patch = session
        .snapshot()
        .patch(|edit| {
            let missing_result = edit.remove_entity_if_present(missing_id, RemovePolicy::Cascade)?;
            assert!(!missing_result, "missing entity removal must return false");

            let existing_result = edit.remove_entity_if_present(entity, RemovePolicy::Cascade)?;
            assert!(existing_result, "existing entity removal must return true");

            let duplicate_result = edit.remove_entity_if_present(entity, RemovePolicy::Cascade)?;
            assert!(!duplicate_result, "subsequent removal must return false");

            Ok(())
        })
        .unwrap();

    let committed = session.commit(patch).await.unwrap();
    assert!(committed.snapshot.entity(entity).is_err());
}

#[tokio::test]
async fn prune_missing_drops_deleted_entity_operations_and_rebases() {
    let mut session = Session::memory().await.unwrap();
    let mut layer_a = None;
    let mut layer_b = None;
    let setup = session
        .snapshot()
        .patch(|edit| {
            let page = edit.add_page(page(), At::End)?;
            let content_a = edit.add_text_content(page, At::End)?;
            edit.set(content_a, &SourceText {
                text: Authored::user("Source A".to_owned()),
                language: None,
            })?;
            let a = edit.add_text_layer(page, At::End, content_a, &TextLayout {
                origin: Origin::User,
                kind: TextLayoutKind::Paragraph,
                angle_degrees: None,
            })?;
            layer_a = Some((content_a, a));

            let content_b = edit.add_text_content(page, At::End)?;
            edit.set(content_b, &SourceText {
                text: Authored::user("Source B".to_owned()),
                language: None,
            })?;
            let b = edit.add_text_layer(page, At::End, content_b, &TextLayout {
                origin: Origin::User,
                kind: TextLayoutKind::Paragraph,
                angle_degrees: None,
            })?;
            layer_b = Some((content_b, b));
            Ok(())
        })
        .unwrap();
    session.commit(setup).await.unwrap();

    let (content_a, layer_a) = layer_a.unwrap();
    let (content_b, _layer_b) = layer_b.unwrap();

    let base_snap = session.snapshot();
    let modify_both = base_snap
        .patch(|edit| {
            edit.observe::<Translation>(content_a)?;
            edit.observe::<Translation>(content_b)?;
            edit.set(
                content_a,
                &Translation {
                    text: Authored::user("Trans A".to_owned()),
                    language: None,
                },
            )?;
            edit.set(
                content_b,
                &Translation {
                    text: Authored::user("Trans B".to_owned()),
                    language: None,
                },
            )?;
            Ok(())
        })
        .unwrap();

    let delete_a = session
        .snapshot()
        .patch(|edit| {
            edit.remove_entity(layer_a, RemovePolicy::Cascade)?;
            edit.remove_entity(content_a, RemovePolicy::Cascade)?;
            Ok(())
        })
        .unwrap();
    session.commit(delete_a).await.unwrap();

    let current = session.snapshot();
    assert!(modify_both.rebase_on(&current).is_err());

    let pruned = modify_both.prune_missing(&current);
    assert!(!pruned.is_empty());
    let rebased = pruned.rebase_on(&current).unwrap();
    let committed = session.commit(rebased).await.unwrap();
    assert_eq!(
        committed.snapshot.text_content(content_b).unwrap().translation().unwrap().unwrap().text.value,
        "Trans B"
    );
}

#[test]
fn schema_1_glossary_entry_binary_decodes_to_schema_2_with_none_character() {
    const SCHEMA_1_ENTRY_BYTES: &[u8] = &[
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

    let entry: GlossaryEntry = revision::from_slice(SCHEMA_1_ENTRY_BYTES)
        .expect("schema-1 GlossaryEntry bytes must decode cleanly into schema-2 struct");

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

#[tokio::test]
async fn revision_one_persisted_project_glossary_upgrades_losslessly() {
    #[revisioned(revision = 1)]
    #[derive(Clone, Debug, PartialEq)]
    struct GlossaryEntryV1 {
        pub id: GlossaryEntryId,
        pub source: String,
        pub translation: Option<String>,
        pub kind: GlossaryKind,
        pub enabled: bool,
        pub confidence: Option<f32>,
        pub occurrence_count: u32,
        pub examples: Vec<String>,
        pub source_origin: GlossaryValueOrigin,
        pub translation_origin: Option<GlossaryValueOrigin>,
        pub present_in_last_scan: bool,
    }

    #[revisioned(revision = 1)]
    #[derive(Clone, Debug, PartialEq)]
    struct GlossaryV1 {
        pub enabled: bool,
        pub source_language: Option<LanguageTag>,
        pub target_language: Option<LanguageTag>,
        pub source_fingerprint: Option<String>,
        pub entries: Vec<GlossaryEntryV1>,
    }

    impl Component for GlossaryV1 {
        const KIND: &'static str = Glossary::KIND;
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1_glossary_project.khrproj");
    let entry_id = GlossaryEntryId::new();

    {
        let mut session = Session::create(&path).await.unwrap();
        let v1_glossary = GlossaryV1 {
            enabled: true,
            source_language: Some(LanguageTag::new("ja").unwrap()),
            target_language: Some(LanguageTag::new("en").unwrap()),
            source_fingerprint: None,
            entries: vec![GlossaryEntryV1 {
                id: entry_id,
                source: "魔法少女".to_owned(),
                translation: Some("Magical Girl".to_owned()),
                kind: GlossaryKind::Term,
                enabled: true,
                confidence: Some(0.95),
                occurrence_count: 12,
                examples: vec!["魔法少女になる".to_owned()],
                source_origin: GlossaryValueOrigin::User,
                translation_origin: Some(GlossaryValueOrigin::User),
                present_in_last_scan: true,
            }],
        };

        let patch = session
            .snapshot()
            .patch(|edit| edit.set_project(&v1_glossary))
            .unwrap();
        session.commit(patch).await.unwrap();
    }

    {
        let session = Session::open(&path).await.unwrap();
        let glossary = session
            .snapshot()
            .project_component::<Glossary>()
            .unwrap()
            .expect("glossary component must exist");

        assert!(glossary.enabled);
        assert_eq!(glossary.source_language.as_ref().map(|l| l.as_str()), Some("ja"));
        assert_eq!(glossary.target_language.as_ref().map(|l| l.as_str()), Some("en"));
        assert_eq!(glossary.entries.len(), 1);

        let entry = &glossary.entries[0];
        assert_eq!(entry.id, entry_id);
        assert_eq!(entry.source, "魔法少女");
        assert_eq!(entry.translation.as_deref(), Some("Magical Girl"));
        assert_eq!(entry.kind, GlossaryKind::Term);
        assert_eq!(entry.character, None);
    }
}

#[tokio::test]
async fn prune_missing_preserves_same_patch_group_layer_and_relation() {
    let mut session = Session::memory().await.unwrap();

    let mut page_id = None;
    let mut content_id = None;
    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(p, At::End)?;
            edit.set(
                content,
                &SourceText {
                    text: Authored::user("セリフ".to_owned()),
                    language: Some(LanguageTag::new("ja")?),
                },
            )?;
            page_id = Some(p);
            content_id = Some(content);
            Ok(())
        })
        .unwrap();
    let base_snapshot = session.commit(setup).await.unwrap().snapshot;
    let page = page_id.unwrap();
    let content = content_id.unwrap();

    assert!(base_snapshot.page(page).unwrap().text_group().unwrap().is_none());

    let mut layer_id = None;
    let patch = base_snapshot
        .patch(|edit| {
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
    let layer = layer_id.unwrap();

    let (pruned, diagnostics) = patch.prune_missing_with_diagnostics(&base_snapshot);
    assert!(
        diagnostics.is_empty(),
        "prune_missing must not drop same-patch dependent operations: {diagnostics:?}"
    );
    assert_eq!(pruned.operations.len(), patch.operations.len());

    let committed_snapshot = session.commit(pruned).await.unwrap().snapshot;

    let page_ref = committed_snapshot.page(page).unwrap();
    let text_group = page_ref.text_group().unwrap().expect("text group must exist");
    assert!(committed_snapshot.component::<TextGroup>(text_group.id()).unwrap().is_some());

    let layer_ref = committed_snapshot.text_layer(layer).expect("text layer must exist");
    assert_eq!(layer_ref.layout().unwrap().kind, TextLayoutKind::Paragraph);

    let presents_relations = committed_snapshot
        .relations_from_as::<Presents>(layer)
        .collect::<Vec<_>>();
    assert_eq!(presents_relations.len(), 1);
    assert_eq!(presents_relations[0].value().target, content);
}

#[tokio::test]
async fn prune_missing_prunes_entire_chain_if_root_page_is_deleted() {
    let mut session = Session::memory().await.unwrap();

    let mut page_id = None;
    let mut content_id = None;
    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(p, At::End)?;
            page_id = Some(p);
            content_id = Some(content);
            Ok(())
        })
        .unwrap();
    let base = session.commit(setup).await.unwrap().snapshot;
    let page = page_id.unwrap();
    let content = content_id.unwrap();

    let patch_a = base
        .patch(|edit| {
            edit.add_text_layer(
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

    let patch_b = base
        .patch(|edit| edit.remove_entity(page, RemovePolicy::Cascade))
        .unwrap();
    let state_b = session.commit(patch_b).await.unwrap().snapshot;

    let (pruned_a, diagnostics) = patch_a.prune_missing_with_diagnostics(&state_b);
    assert!(!diagnostics.is_empty(), "expected diagnostics when root page deleted");
    assert_eq!(
        pruned_a.operations.len(),
        0,
        "all operations must be pruned when root page was deleted"
    );
}

#[tokio::test]
async fn speaker_origin_provenance_is_preserved() {
    let mut session = Session::memory().await.unwrap();
    let entry_id = GlossaryEntryId::new();
    let mut layer_id = None;

    let setup = session
        .snapshot()
        .patch(|edit| {
            let p = edit.add_page(page(), At::End)?;
            let content = edit.add_text_content(p, At::End)?;
            let layer = edit.add_text_layer(
                p,
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
    let committed = session.commit(setup).await.unwrap();
    let layer = layer_id.unwrap();

    let speaker_gen = Generation::new(ProducerId::new("dev.koharu.test.speaker").unwrap());
    let patch = committed
        .snapshot
        .patch(|edit| edit.set_speaker(layer, Some(entry_id), Origin::Generated(speaker_gen.clone())))
        .unwrap();
    let snap = session.commit(patch).await.unwrap().snapshot;
    let speaker = snap.component::<Speaker>(layer).unwrap().unwrap();
    assert_eq!(speaker.speaker, Some(entry_id));
    assert_eq!(speaker.origin, Origin::Generated(speaker_gen));

    let patch = snap
        .patch(|edit| edit.set_speaker(layer, Some(entry_id), Origin::User))
        .unwrap();
    let snap = session.commit(patch).await.unwrap().snapshot;
    let speaker = snap.component::<Speaker>(layer).unwrap().unwrap();
    assert_eq!(speaker.speaker, Some(entry_id));
    assert_eq!(speaker.origin, Origin::User);
}

#[test]
fn glossary_entry_rev2_and_character_profile_rev1_decode_with_defaults() {
    #[revision::revisioned(revision = 1)]
    #[derive(Clone, Debug, PartialEq)]
    struct CharacterProfileV1 {
        id: GlossaryEntryId,
        gender: CharacterGender,
        first_person: Option<String>,
        second_person: Option<String>,
        tone: Option<String>,
        custom_instructions: Option<String>,
    }

    #[revision::revisioned(revision = 2)]
    #[derive(Clone, Debug, PartialEq)]
    struct GlossaryEntryV2 {
        id: GlossaryEntryId,
        source: String,
        translation: Option<String>,
        kind: GlossaryKind,
        enabled: bool,
        confidence: Option<f32>,
        occurrence_count: u32,
        examples: Vec<String>,
        source_origin: GlossaryValueOrigin,
        translation_origin: Option<GlossaryValueOrigin>,
        present_in_last_scan: bool,
        #[revision(start = 2)]
        character: Option<CharacterProfileV1>,
    }

    let id = GlossaryEntryId::new();
    let legacy = GlossaryEntryV2 {
        id,
        source: "春香".to_owned(),
        translation: Some("Haruka".to_owned()),
        kind: GlossaryKind::Person,
        enabled: true,
        confidence: Some(0.95),
        occurrence_count: 3,
        examples: vec!["春香さん".to_owned()],
        source_origin: GlossaryValueOrigin::Detected,
        translation_origin: Some(GlossaryValueOrigin::User),
        present_in_last_scan: true,
        character: Some(CharacterProfileV1 {
            id,
            gender: CharacterGender::Female,
            first_person: Some("私".to_owned()),
            second_person: None,
            tone: None,
            custom_instructions: None,
        }),
    };


    let bytes = revision::to_vec(&legacy).unwrap();
    let decoded =
        revision::from_slice::<GlossaryEntry>(&bytes)
            .unwrap();
    assert!(decoded.forbidden_variants.is_empty());
    assert_eq!(
        decoded.character.as_ref().and_then(|c| c.honorific_policy.clone()),
        None
    );
}

