use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};

use anyhow::{Result, bail};
use koharu_scene::{
    EntityId, FitsTo, FlowsIn, Geometry, Inside, Presents, RecognizedFrom, RelationSpec, Snapshot,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::Stage;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize, Type)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    fn validate(self) -> Result<()> {
        if self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width > 0.0
            && self.height > 0.0
        {
            Ok(())
        } else {
            bail!("region bounds must be finite and non-empty")
        }
    }

    pub(crate) fn intersects(self, geometry: &Geometry) -> bool {
        let Some((min_x, min_y, max_x, max_y)) = geometry_extents(geometry) else {
            return false;
        };
        min_x < self.x + self.width
            && max_x > self.x
            && min_y < self.y + self.height
            && max_y > self.y
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize, Type)]
#[serde(tag = "scope", content = "value", rename_all = "snake_case")]
pub enum Scope {
    #[default]
    Project,
    Pages(Vec<EntityId>),
    Region {
        page: EntityId,
        bounds: Bounds,
    },
    Entities(Vec<EntityId>),
}

#[derive(Clone, Debug)]
pub(crate) struct NormalizedScope {
    pages: Arc<[EntityId]>,
    entities: Option<Arc<BTreeSet<EntityId>>>,
    region: Option<(EntityId, Bounds)>,
}

impl NormalizedScope {
    pub(crate) fn new(snapshot: &Snapshot, scope: &Scope, stages: &[Stage]) -> Result<Self> {
        if matches!(scope, Scope::Entities(_))
            && stages
                .iter()
                .any(|stage| matches!(stage, Stage::Detection | Stage::Inpainting))
        {
            bail!("detection and inpainting do not support entity-only scope");
        }

        match scope {
            Scope::Project => Ok(Self {
                pages: snapshot.pages().map(|page| page.id()).collect(),
                entities: None,
                region: None,
            }),
            Scope::Pages(requested) => {
                if requested.is_empty() {
                    bail!("page scope is empty");
                }
                let requested = requested.iter().copied().collect::<BTreeSet<_>>();
                for id in &requested {
                    snapshot.page(*id)?;
                }
                let pages = snapshot
                    .pages()
                    .map(|page| page.id())
                    .filter(|id| requested.contains(id))
                    .collect::<Arc<[_]>>();
                Ok(Self {
                    pages,
                    entities: None,
                    region: None,
                })
            }
            Scope::Region { page, bounds } => {
                snapshot.page(*page)?;
                bounds.validate()?;
                Ok(Self {
                    pages: Arc::from([*page]),
                    entities: None,
                    region: Some((*page, *bounds)),
                })
            }
            Scope::Entities(requested) => {
                if requested.is_empty() {
                    bail!("entity scope is empty");
                }
                let requested = requested.iter().copied().collect::<BTreeSet<_>>();
                let mut page_set = BTreeSet::new();
                for entity in &requested {
                    snapshot.entity(*entity)?;
                    page_set.insert(containing_page(snapshot, *entity)?);
                }
                let mut hierarchy = BTreeSet::new();
                for entity in requested {
                    hierarchy.extend(snapshot.subtree(entity)?.map(|entity| entity.id()));
                }
                let entities = semantic_closure(snapshot, hierarchy);
                let pages = snapshot
                    .pages()
                    .map(|page| page.id())
                    .filter(|id| page_set.contains(id))
                    .collect::<Arc<[_]>>();
                Ok(Self {
                    pages,
                    entities: Some(Arc::new(entities)),
                    region: None,
                })
            }
        }
    }

    pub(crate) fn pages(&self) -> &[EntityId] {
        &self.pages
    }

    pub(crate) fn entities(&self) -> Option<Arc<BTreeSet<EntityId>>> {
        self.entities.clone()
    }

    pub(crate) fn region(&self, page: EntityId) -> Option<Bounds> {
        debug_assert!(self.pages.contains(&page));
        self.region
            .and_then(|(candidate, bounds)| (candidate == page).then_some(bounds))
    }
}

fn semantic_closure(snapshot: &Snapshot, mut entities: BTreeSet<EntityId>) -> BTreeSet<EntityId> {
    let mut pending = entities.iter().copied().collect::<VecDeque<_>>();
    while let Some(entity) = pending.pop_front() {
        for relation in snapshot
            .relations_from(entity, None)
            .chain(snapshot.relations_to(entity, None))
        {
            let value = relation.value();
            if ![
                Presents::KIND,
                RecognizedFrom::KIND,
                FitsTo::KIND,
                FlowsIn::KIND,
                Inside::KIND,
            ]
            .contains(&value.kind.as_str())
            {
                continue;
            }
            for related in [value.source, value.target] {
                if entities.insert(related) {
                    pending.push_back(related);
                }
            }
        }
    }
    entities
}

pub(crate) fn resolve_geometry(snapshot: &Snapshot, entity: EntityId) -> Result<Option<Geometry>> {
    if let Some(geometry) = snapshot.component::<Geometry>(entity)? {
        return Ok(Some(geometry));
    }
    for relation in snapshot.relations_from_as::<Presents>(entity) {
        let content = relation.value().target;
        if let Some(geometry) = snapshot.component::<Geometry>(content)? {
            return Ok(Some(geometry));
        }
        for recognized in snapshot.relations_from_as::<RecognizedFrom>(content) {
            let region = recognized.value().target;
            if let Some(geometry) = snapshot.component::<Geometry>(region)? {
                return Ok(Some(geometry));
            }
        }
        for recognized in snapshot.relations_to_as::<RecognizedFrom>(content) {
            let region = recognized.value().source;
            if let Some(geometry) = snapshot.component::<Geometry>(region)? {
                return Ok(Some(geometry));
            }
        }
    }
    for relation in snapshot.relations_to_as::<Presents>(entity) {
        let layer = relation.value().source;
        if let Some(geometry) = snapshot.component::<Geometry>(layer)? {
            return Ok(Some(geometry));
        }
    }
    for recognized in snapshot.relations_from_as::<RecognizedFrom>(entity) {
        let region = recognized.value().target;
        if let Some(geometry) = snapshot.component::<Geometry>(region)? {
            return Ok(Some(geometry));
        }
    }
    for recognized in snapshot.relations_to_as::<RecognizedFrom>(entity) {
        let region = recognized.value().source;
        if let Some(geometry) = snapshot.component::<Geometry>(region)? {
            return Ok(Some(geometry));
        }
    }
    for relation in snapshot.relations_from_as::<FitsTo>(entity) {
        let target = relation.value().target;
        if let Some(geometry) = snapshot.component::<Geometry>(target)? {
            return Ok(Some(geometry));
        }
    }
    Ok(None)
}

pub(crate) fn contains_entity(
    snapshot: &Snapshot,
    page: EntityId,
    entities: Option<&BTreeSet<EntityId>>,
    region: Option<Bounds>,
    entity: EntityId,
) -> Result<bool> {
    if containing_page(snapshot, entity)? != page {
        return Ok(false);
    }
    if let Some(entities) = entities {
        return Ok(entities.contains(&entity));
    }
    let Some(bounds) = region else {
        return Ok(true);
    };
    Ok(resolve_geometry(snapshot, entity)?
        .is_some_and(|geometry| bounds.intersects(&geometry)))
}

fn containing_page(snapshot: &Snapshot, mut entity: EntityId) -> Result<EntityId> {
    loop {
        if snapshot.page(entity).is_ok() {
            return Ok(entity);
        }
        entity = snapshot
            .parent(entity)?
            .ok_or_else(|| anyhow::anyhow!("entity is not contained by a page"))?;
    }
}

pub(crate) fn geometry_extents(geometry: &Geometry) -> Option<(f64, f64, f64, f64)> {
    let first = geometry.points.first()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.x;
    let mut max_y = first.y;
    for point in &geometry.points[1..] {
        min_x = min_x.min(point.x);
        min_y = min_y.min(point.y);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    Some((min_x, min_y, max_x, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use koharu_scene::{At, PageDraft, Session, TextLayout, TextLayoutKind, TextRegion};

    #[tokio::test]
    async fn resolve_geometry_finds_region_via_presents_and_recognized_from() {
        let mut session = Session::memory().await.unwrap();
        let mut layer_id = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 200.0, 200.0), At::End)?;
                let region = edit.add_analysis_region::<TextRegion>(
                    page,
                    At::End,
                    &Geometry::rectangle(10.0, 20.0, 30.0, 40.0),
                    None,
                )?;
                let content = edit.add_text_content(page, At::End)?;
                edit.relate::<RecognizedFrom>(content, region)?;
                let layer = edit.add_text_layer(
                    page,
                    At::End,
                    content,
                    &TextLayout {
                        origin: koharu_scene::Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                layer_id = Some(layer);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let resolved = resolve_geometry(&snapshot, layer_id.unwrap()).unwrap();
        assert!(resolved.is_some());
        let extents = geometry_extents(&resolved.unwrap()).unwrap();
        assert_eq!(extents, (10.0, 20.0, 40.0, 60.0));
    }
}
