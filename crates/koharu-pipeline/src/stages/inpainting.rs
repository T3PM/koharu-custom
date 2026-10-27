use std::{
    collections::BTreeMap,
    io::Cursor,
    sync::{Arc, Mutex},
};

use anyhow::{Result, anyhow, bail, ensure};
use async_trait::async_trait;
use image::{
    DynamicImage, GenericImageView as _, GrayImage, ImageBuffer, ImageFormat, Luma, Rgb, RgbImage,
    Rgba, RgbaImage,
};
use imageproc::{
    distance_transform::Norm,
    morphology::dilate,
    region_labelling::{Connectivity, connected_components},
};
use koharu_ml::{
    aot_inpainting::AotInpainting,
    flux2_klein::{Flux2KleinInpaint, Flux2KleinInpaintOptions},
    lama::{InpaintRequest, LaMa},
    rorem_mixed::{DEFAULT_NEGATIVE_PROMPT, DEFAULT_PROMPT, RoremMixed, RoremMixedOptions},
};
use koharu_scene::{
    AssetInput, AssetMetadata, AssetRole, At, BubbleRegion, EntityId, EntityOrigin, FlowsIn,
    Generation, Geometry, Origin, RasterLayer, RasterLayerKind, Region, RegionSpec, Snapshot,
    TextLayout, TextRegion, Typography, WritingMode,
};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
    StageInput, StageProcessor,
    detection::{luma_median_and_iqr, luma_u8, synthesize_halo_stroke},
    finish, generation,
};
use crate::{InpaintingModel, ModelCell};

const PRODUCER: &str = "dev.koharu.pipeline.inpainting";
const HALO_REFINEMENT_OUTER_RING: u32 = 4;
const HALO_REFINEMENT_LOW_CONTRAST_DELTA: u8 = 96;
const HALO_REFINEMENT_HIGH_VARIANCE_IQR: u8 = 40;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct Flux2KleinConfig {
    pub prompt: String,
}

impl Default for Flux2KleinConfig {
    fn default() -> Self {
        Self {
            prompt: "Remove the text and reconstruct the background.".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct RoremMixedConfig {
    pub prompt: String,
    pub negative_prompt: String,
}

impl Default for RoremMixedConfig {
    fn default() -> Self {
        Self {
            prompt: DEFAULT_PROMPT.to_owned(),
            negative_prompt: DEFAULT_NEGATIVE_PROMPT.to_owned(),
        }
    }
}

pub(super) struct Processor {
    config: InpaintingModel,
    device: koharu_ml::Device,
    model: ModelCell<Model>,
}

impl Processor {
    pub(super) fn new(config: InpaintingModel, device: koharu_ml::Device) -> Result<Self> {
        match &config {
            InpaintingModel::LaMa {} | InpaintingModel::AotInpainting {} => {}
            InpaintingModel::Flux2Klein(settings) => {
                ensure!(
                    !settings.prompt.contains('\0'),
                    "FLUX.2 prompt contains NUL"
                );
            }
            InpaintingModel::RoremMixed(settings) => {
                ensure!(
                    !settings.prompt.contains('\0') && !settings.negative_prompt.contains('\0'),
                    "RORem prompt contains NUL"
                );
            }
        }

        Ok(Self {
            config,
            device,
            model: ModelCell::new(),
        })
    }
}

#[async_trait]
impl StageProcessor for Processor {
    fn model(&self) -> &'static str {
        match self.config {
            InpaintingModel::LaMa {} => "lama",
            InpaintingModel::AotInpainting {} => "aot-inpainting",
            InpaintingModel::Flux2Klein(_) => "flux2-klein",
            InpaintingModel::RoremMixed(_) => "rorem-mixed",
        }
    }

    fn skip(&self, input: &StageInput) -> Result<bool> {
        if input.inpainting_mask.is_some() {
            return Ok(false);
        }
        let source = AssetRole::new("source")?;
        for entity in input.scene.children(input.page)? {
            if input
                .scene
                .component::<RasterLayer>(entity)?
                .is_some_and(|layer| layer.kind == RasterLayerKind::Cleanup)
                && input.scene.asset(entity, &source)?.is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn unload(&self) -> bool {
        self.model.unload()
    }

    async fn load(&self) -> Result<()> {
        self.model
            .ensure(|| Model::load(self.device.clone(), &self.config))
            .await
    }

    async fn process(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        self.model
            .lock()
            .await
            .as_ref()
            .ok_or_else(|| anyhow!("inpainting model is not loaded"))?
            .run(input)
            .await
    }
}

enum Model {
    LaMa(Arc<Mutex<LaMa>>),
    Aot(Arc<Mutex<AotInpainting>>),
    Flux {
        model: Arc<Mutex<Flux2KleinInpaint>>,
        config: Flux2KleinConfig,
    },
    Rorem {
        model: Arc<Mutex<RoremMixed>>,
        config: RoremMixedConfig,
    },
}

impl Model {
    async fn load(device: koharu_ml::Device, config: &InpaintingModel) -> Result<Self> {
        match config {
            InpaintingModel::LaMa {} => {
                Ok(Self::LaMa(Arc::new(Mutex::new(LaMa::load(device).await?))))
            }
            InpaintingModel::AotInpainting {} => Ok(Self::Aot(Arc::new(Mutex::new(
                AotInpainting::load(device).await?,
            )))),
            InpaintingModel::Flux2Klein(config) => Ok(Self::Flux {
                model: Arc::new(Mutex::new(Flux2KleinInpaint::load(device).await?)),
                config: config.clone(),
            }),
            InpaintingModel::RoremMixed(config) => Ok(Self::Rorem {
                model: Arc::new(Mutex::new(RoremMixed::load(device).await?)),
                config: config.clone(),
            }),
        }
    }

    async fn run(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        let mut prepared = prepare(&input).await?;
        if prepared.mask.as_raw().iter().all(|value| *value == 0) {
            return finish(input.scene.edit());
        }
        let mask = prepared.mask.clone();
        let original = prepared.original.clone();
        let cleanup = prepared.cleanup.take();
        let cleanup_entity = prepared.cleanup_entity;
        let (model_name, image) = match self {
            Self::LaMa(model) => {
                let model = model.clone();
                (
                    "lama",
                    tokio_rayon::spawn(move || -> Result<DynamicImage> {
                        let model = model
                            .lock()
                            .map_err(|_| anyhow!("LaMa model lock is poisoned"))?;
                        inpaint_tiled(
                            &prepared.image,
                            &prepared.mask,
                            &prepared.text_mask,
                            &prepared.flat_fill_regions,
                            |image, mask| {
                                Ok(DynamicImage::ImageRgb8(model.inference(
                                    image,
                                    mask,
                                    &InpaintRequest::default(),
                                )?))
                            },
                        )
                    })
                    .await?,
                )
            }
            Self::Aot(model) => {
                let model = model.clone();
                (
                    "aot-inpainting",
                    tokio_rayon::spawn(move || -> Result<DynamicImage> {
                        let model = model
                            .lock()
                            .map_err(|_| anyhow!("AOT model lock is poisoned"))?;
                        inpaint_tiled(
                            &prepared.image,
                            &prepared.mask,
                            &prepared.text_mask,
                            &prepared.flat_fill_regions,
                            |image, mask| {
                                Ok(DynamicImage::ImageRgb8(model.inference(image, mask)?))
                            },
                        )
                    })
                    .await?,
                )
            }
            Self::Flux { model, config } => {
                let model = model.clone();
                let config = config.clone();
                (
                    "flux2-klein",
                    tokio_rayon::spawn(move || -> Result<DynamicImage> {
                        let model = model
                            .lock()
                            .map_err(|_| anyhow!("FLUX model lock is poisoned"))?;
                        inpaint_tiled(
                            &prepared.image,
                            &prepared.mask,
                            &prepared.text_mask,
                            &prepared.flat_fill_regions,
                            |image, mask| {
                                model.inference(
                                    &config.prompt,
                                    image,
                                    None,
                                    &DynamicImage::ImageLuma8(mask.clone()),
                                    &Flux2KleinInpaintOptions::default(),
                                )
                            },
                        )
                    })
                    .await?,
                )
            }
            Self::Rorem { model, config } => {
                let model = model.clone();
                let config = config.clone();
                (
                    "rorem-mixed",
                    tokio_rayon::spawn(move || -> Result<DynamicImage> {
                        let model = model
                            .lock()
                            .map_err(|_| anyhow!("RORem model lock is poisoned"))?;
                        inpaint_tiled(
                            &prepared.image,
                            &prepared.mask,
                            &prepared.text_mask,
                            &prepared.flat_fill_regions,
                            |image, mask| {
                                Ok(DynamicImage::ImageRgb8(model.inference(
                                    image,
                                    mask,
                                    &config.prompt,
                                    &config.negative_prompt,
                                    &RoremMixedOptions::default(),
                                )?))
                            },
                        )
                    })
                    .await?,
                )
            }
        };
        let page = input.page;
        let manual = input.inpainting_mask.is_some();
        let generation = generation(PRODUCER, model_name)?;
        let image = image.to_rgba8();
        if image.dimensions() != original.dimensions() || image.dimensions() != mask.dimensions() {
            bail!("inpainted image dimensions do not match page {page}");
        }
        let mut overlay = if manual || input.region.is_some() {
            cleanup.unwrap_or_else(|| RgbaImage::new(image.width(), image.height()))
        } else {
            RgbaImage::new(image.width(), image.height())
        };
        for (x, y, target) in overlay.enumerate_pixels_mut() {
            if mask.get_pixel(x, y)[0] < 127 {
                continue;
            }
            let generated = image.get_pixel(x, y);
            // Keep the complete cleanup mask opaque. Transparent texels next to
            // edited pixels let linear canvas sampling reveal the artwork below.
            *target = Rgba([generated[0], generated[1], generated[2], 255]);
        }
        let mut bytes = Cursor::new(Vec::new());
        let width = overlay.width();
        let height = overlay.height();
        DynamicImage::ImageRgba8(overlay).write_to(&mut bytes, ImageFormat::Png)?;
        let png_bytes = Arc::<[u8]>::from(bytes.into_inner());
        let patch = input.scene.patch(|edit| {
            edit.observe_assets(page)?;
            if let Some(entity) = cleanup_entity {
                edit.observe::<RasterLayer>(entity)?;
                edit.observe_assets(entity)?;
            }
            let cleanup_entity = if let Some(entity) = cleanup_entity {
                if manual {
                    let mut layer = input
                        .scene
                        .component::<RasterLayer>(entity)?
                        .ok_or_else(|| {
                            koharu_scene::Error::Invalid(
                                "cleanup entity has no raster layer component".to_owned(),
                            )
                        })?;
                    let generated = layer.origin != Origin::User
                        || input
                            .scene
                            .component::<EntityOrigin>(entity)?
                            .is_some_and(|origin| origin.origin != Origin::User);
                    if generated {
                        edit.promote_entity_to_user(entity)?;
                        layer.origin = Origin::User;
                        edit.set(entity, &layer)?;
                    }
                } else {
                    let mut layer = input
                        .scene
                        .component::<RasterLayer>(entity)?
                        .ok_or_else(|| {
                            koharu_scene::Error::Invalid(
                                "cleanup entity has no raster layer component".to_owned(),
                            )
                        })?;
                    if !matches!(layer.origin, Origin::User) {
                        layer.origin = Origin::Generated(generation.clone());
                        edit.set(entity, &layer)?;
                    }
                }
                entity
            } else {
                let entity = edit.add_entity(page, At::Start)?;
                edit.set(
                    entity,
                    &RasterLayer {
                        origin: if manual {
                            Origin::User
                        } else {
                            Origin::Generated(generation.clone())
                        },
                        name: "Cleanup".to_owned(),
                        kind: RasterLayerKind::Cleanup,
                    },
                )?;
                entity
            };
            edit.set_asset(
                cleanup_entity,
                &AssetRole::new("source")?,
                AssetInput::new(
                    png_bytes,
                    "image/png",
                    AssetMetadata {
                        width: Some(width),
                        height: Some(height),
                        attributes: BTreeMap::new(),
                    },
                ),
            )?;
            if !manual {
                refine_floating_text_halos(&input.scene, page, &image, edit, &generation)?;
            }
            Ok(())
        })?;
        Ok(patch)
    }
}

pub(crate) fn refine_floating_text_halos(
    scene: &Snapshot,
    page: EntityId,
    inpainted: &RgbaImage,
    edit: &mut koharu_scene::Edit,
    _generation: &Generation,
) -> koharu_scene::Result<()> {
    let width = inpainted.width();
    let height = inpainted.height();
    if width == 0 || height == 0 {
        return Ok(());
    }
    let mut layer_ids = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(group) = scene.page(page)?.text_group()? {
        for layer in group.text_layers()? {
            if seen.insert(layer.id()) {
                layer_ids.push(layer.id());
            }
        }
    }
    for entity in scene.descendants(page)? {
        let id = entity.id();
        if scene.component::<TextLayout>(id)?.is_some() && seen.insert(id) {
            layer_ids.push(id);
        }
    }

    for layer_id in layer_ids {
        if scene.relation_from::<FlowsIn>(layer_id)?.is_some() {
            continue;
        }
        let Some(typography) = scene.component::<Typography>(layer_id)? else {
            continue;
        };
        if matches!(typography.origin, Origin::User) {
            continue;
        }
        let Some(geometry) = crate::scope::resolve_geometry(scene, layer_id)
            .map_err(|err| koharu_scene::Error::Invalid(err.to_string()))?
        else {
            continue;
        };
        let Some((min_x, min_y, max_x, max_y)) = crate::scope::geometry_extents(&geometry) else {
            continue;
        };
        let left = (min_x.floor() as i64 - i64::from(HALO_REFINEMENT_OUTER_RING))
            .clamp(0, i64::from(width)) as u32;
        let top = (min_y.floor() as i64 - i64::from(HALO_REFINEMENT_OUTER_RING))
            .clamp(0, i64::from(height)) as u32;
        let right = (max_x.ceil() as i64 + i64::from(HALO_REFINEMENT_OUTER_RING))
            .clamp(0, i64::from(width)) as u32;
        let bottom = (max_y.ceil() as i64 + i64::from(HALO_REFINEMENT_OUTER_RING))
            .clamp(0, i64::from(height)) as u32;
        if right <= left || bottom <= top {
            continue;
        }
        let mut samples = Vec::with_capacity(((right - left) * (bottom - top)) as usize);
        for y in top..bottom {
            for x in left..right {
                let pixel = inpainted.get_pixel(x, y).0;
                samples.push([pixel[0], pixel[1], pixel[2]]);
            }
        }
        if samples.is_empty() {
            continue;
        }
        let fill_color = typography.color.unwrap_or([0, 0, 0, u8::MAX]);
        let fill_luma = luma_u8([fill_color[0], fill_color[1], fill_color[2]]);
        let (bg_luma, iqr) = luma_median_and_iqr(&samples);
        if typography.stroke_width.is_none()
            || fill_luma.abs_diff(bg_luma) < HALO_REFINEMENT_LOW_CONTRAST_DELTA
            || iqr >= HALO_REFINEMENT_HIGH_VARIANCE_IQR
        {
            let box_w = (max_x - min_x).max(1.0) as f32;
            let box_h = (max_y - min_y).max(1.0) as f32;
            let font_size = typography.size.unwrap_or_else(|| {
                let extent = match typography.writing_mode {
                    Some(WritingMode::Vertical) => box_w,
                    Some(WritingMode::Horizontal) => box_h,
                    None => box_w.min(box_h),
                };
                extent.clamp(12.0, 96.0)
            });
            let (stroke_color, stroke_width, outer_stroke) =
                synthesize_halo_stroke(fill_color, &samples, font_size);
            let mut updated_typography = typography.clone();
            updated_typography.stroke_color = stroke_color;
            updated_typography.stroke_width = stroke_width;
            updated_typography.set_outer_stroke(outer_stroke);
            updated_typography.origin = typography.origin;
            edit.set(layer_id, &updated_typography)?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct FlatFillRegion {
    bounds: [u32; 4],
    polygon: Vec<(f32, f32)>,
}

fn flat_fill_regions(input: &StageInput, width: u32, height: u32) -> Result<Vec<FlatFillRegion>> {
    let mut regions = Vec::new();
    for entity in input.scene.descendants(input.page)? {
        let id = entity.id();
        let is_bubble = input
            .scene
            .component::<Region>(id)?
            .is_some_and(|region| region.kind == BubbleRegion::kind());
        if !is_bubble {
            continue;
        }
        let Some(geometry) = input.scene.component::<Geometry>(id)? else {
            continue;
        };
        let polygon = geometry
            .points
            .iter()
            .map(|point| (point.x as f32, point.y as f32))
            .collect::<Vec<_>>();
        if polygon.len() < 3 {
            continue;
        }
        let (mut left, mut top) = (f32::INFINITY, f32::INFINITY);
        let (mut right, mut bottom) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for &(x, y) in &polygon {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
        let bounds = [
            left.floor().clamp(0.0, width as f32) as u32,
            top.floor().clamp(0.0, height as f32) as u32,
            right.ceil().clamp(0.0, width as f32) as u32,
            bottom.ceil().clamp(0.0, height as f32) as u32,
        ];
        if bounds[2] > bounds[0] && bounds[3] > bounds[1] {
            regions.push(FlatFillRegion { bounds, polygon });
        }
    }
    Ok(regions)
}

struct InpaintInput {
    image: Arc<DynamicImage>,
    original: Arc<DynamicImage>,
    cleanup_entity: Option<koharu_scene::EntityId>,
    cleanup: Option<RgbaImage>,
    mask: GrayImage,
    text_mask: GrayImage,
    flat_fill_regions: Vec<FlatFillRegion>,
}

fn is_sfx_entity(scene: &koharu_scene::Snapshot, id: koharu_scene::EntityId) -> bool {
    const SFX_ROLE: &str = "dev.koharu.text.sfx";
    let has_sfx_role = |content_id: koharu_scene::EntityId| -> bool {
        scene
            .component::<koharu_scene::TextRole>(content_id)
            .ok()
            .flatten()
            .is_some_and(|r| r.role == SFX_ROLE)
    };
    if has_sfx_role(id) {
        return true;
    }
    if let Ok(Some(presents)) = scene.relation_from::<koharu_scene::Presents>(id) {
        if has_sfx_role(presents.value().target) {
            return true;
        }
    }
    for fits in scene.relations_to_as::<koharu_scene::FitsTo>(id) {
        if let Ok(Some(presents)) = scene.relation_from::<koharu_scene::Presents>(fits.value().source) {
            if has_sfx_role(presents.value().target) {
                return true;
            }
        }
    }
    for recognized in scene.relations_to_as::<koharu_scene::RecognizedFrom>(id) {
        if has_sfx_role(recognized.value().source) {
            return true;
        }
    }
    false
}

fn clip_to_region(mask: &mut GrayImage, text_mask: &mut GrayImage, bounds: crate::Bounds) {
    for (x, y, pixel) in mask.enumerate_pixels_mut() {
        if f64::from(x + 1) <= bounds.x
            || f64::from(y + 1) <= bounds.y
            || f64::from(x) >= bounds.x + bounds.width
            || f64::from(y) >= bounds.y + bounds.height
        {
            *pixel = Luma([0]);
            text_mask.put_pixel(x, y, Luma([0]));
        }
    }
}

pub(crate) fn fill_polygon_on_mask(mask: &mut GrayImage, points: &[koharu_scene::Point]) {
    let n = points.len();
    if n < 3 {
        return;
    }
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }
    let width = mask.width();
    let height = mask.height();
    if width == 0 || height == 0 {
        return;
    }
    let start_x = min_x.floor().clamp(0.0, width as f64) as u32;
    let end_x = max_x.ceil().clamp(0.0, width as f64) as u32;
    let start_y = min_y.floor().clamp(0.0, height as f64) as u32;
    let end_y = max_y.ceil().clamp(0.0, height as f64) as u32;

    if start_x >= end_x || start_y >= end_y {
        return;
    }

    let stride = width as usize;
    let raw: &mut [u8] = mask.as_mut();
    let mut intersections = Vec::with_capacity(16);

    for y in start_y..end_y {
        let py = y as f64 + 0.5;
        intersections.clear();

        let mut j = n - 1;
        for i in 0..n {
            let pi = &points[i];
            let pj = &points[j];
            if (pi.y > py) != (pj.y > py) {
                let x_int = (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x;
                intersections.push(x_int);
            }
            j = i;
        }

        intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let row_start = (y as usize) * stride;
        for chunk in intersections.chunks_exact(2) {
            let x0 = chunk[0];
            let x1 = chunk[1];
            if x1 <= x0 {
                continue;
            }
            let span_start = (((x0 - 0.5).ceil() as i64).max(0) as u32).clamp(start_x, end_x);
            let span_end = (((x1 - 0.5).ceil() as i64).max(0) as u32).clamp(start_x, end_x);

            if span_start < span_end {
                let from = row_start + span_start as usize;
                let to = row_start + span_end as usize;
                raw[from..to].fill(255);
            }
        }
    }
}

async fn prepare(input: &StageInput) -> Result<InpaintInput> {
    let page = input.page;
    let original = input
        .images
        .get(&input.scene, page, "source")
        .await?
        .ok_or_else(|| anyhow!("page {page} has no source image"))?;
    let cleanup_entity = input.scene.children(page)?.find(|entity| {
        input
            .scene
            .component::<RasterLayer>(*entity)
            .ok()
            .flatten()
            .is_some_and(|layer| layer.kind == RasterLayerKind::Cleanup)
    });
    let cleanup = if let Some(entity) = cleanup_entity {
        input
            .images
            .get(&input.scene, entity, "source")
            .await?
            .map(|image| image.to_rgba8())
    } else {
        None
    };
    if cleanup
        .as_ref()
        .is_some_and(|image| image.dimensions() != original.dimensions())
    {
        bail!("cleanup layer dimensions do not match page {page}");
    }
    let source = if input.inpainting_mask.is_some() || input.region.is_some() {
        if let Some(cleanup) = cleanup.as_ref() {
            let mut composite = original.to_rgba8();
            image::imageops::overlay(&mut composite, cleanup, 0, 0);
            Arc::new(DynamicImage::ImageRgba8(composite))
        } else {
            original.clone()
        }
    } else {
        original.clone()
    };
    let mut mask = GrayImage::new(source.width(), source.height());
    let mut text_mask = GrayImage::new(source.width(), source.height());
    let mut sfx_mask = GrayImage::new(source.width(), source.height());
    let mut has_sfx = false;

    if let Some(transient) = &input.inpainting_mask {
        let layer = image::load_from_memory(&transient.png)?.to_luma8();
        if layer.dimensions() != mask.dimensions() {
            bail!("inpainting mask dimensions do not match page {page}");
        }
        mask = layer;
    } else {
        for role in ["text-mask", "coo-mask"] {
            if let Some(image) = input.images.get(&input.scene, page, role).await? {
                let layer = image.to_luma8();
                if layer.dimensions() != mask.dimensions() {
                    bail!("{role} dimensions do not match page {page}");
                }
                for (target, source) in mask.iter_mut().zip(layer.as_raw()) {
                    *target = (*target).max(*source);
                }
                if role == "text-mask" {
                    text_mask = layer;
                }
            }
        }

        let mut visited = std::collections::HashSet::new();
        let mut process_entity = |id: koharu_scene::EntityId| -> Result<()> {
            if !visited.insert(id) {
                return Ok(());
            }
            if !input.contains_entity(id)? {
                return Ok(());
            }
            let is_text = input.scene.component::<TextLayout>(id)?.is_some()
                || input
                    .scene
                    .component::<Region>(id)?
                    .is_some_and(|r| r.kind == TextRegion::kind());
            if !is_text {
                return Ok(());
            }
            if let Some(geometry) = crate::scope::resolve_geometry(&input.scene, id)? {
                if is_sfx_entity(&input.scene, id) {
                    has_sfx = true;
                    fill_polygon_on_mask(&mut sfx_mask, &geometry.points);
                } else {
                    fill_polygon_on_mask(&mut mask, &geometry.points);
                    fill_polygon_on_mask(&mut text_mask, &geometry.points);
                }
            }
            Ok(())
        };

        if let Ok(Some(group)) = input.scene.page(page)?.text_group() {
            if let Ok(layers) = group.text_layers() {
                for layer in layers {
                    process_entity(layer.id())?;
                }
            }
        }
        for entity in input.scene.descendants(page)? {
            process_entity(entity.id())?;
        }

        if has_sfx && mask.width() > 0 && mask.height() > 0 {
            let sfx_radius = ((mask.width().max(mask.height()) as f32 / 1024.0) * 3.0)
                .round()
                .clamp(1.0, 16.0) as u8;
            let dilated_sfx = dilate(&sfx_mask, Norm::L2, sfx_radius);
            for ((m, tm), s) in mask.iter_mut().zip(text_mask.iter_mut()).zip(dilated_sfx.as_raw()) {
                *m = (*m).max(*s);
                *tm = (*tm).max(*s);
            }
        }
    }
    if let Some(bounds) = input.region {
        clip_to_region(&mut mask, &mut text_mask, bounds);
    }
    let flat_fill_regions = flat_fill_regions(input, source.width(), source.height())?;
    Ok(InpaintInput {
        image: source,
        original,
        cleanup_entity,
        cleanup,
        mask,
        text_mask,
        flat_fill_regions,
    })
}

const TILE_SIZE: u32 = 512;
const TILE_CONTEXT: u32 = 128;
const UNIFORM_BACKGROUND_MIN_PIXELS: usize = 16;
const FLAT_FILL_EDGE_MARGIN: f32 = 3.0;

#[derive(Clone, Debug, PartialEq, Eq)]
struct InpaintTile {
    components: Vec<u32>,
    core: [u32; 4],
    crop: [u32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MaskComponent {
    label: u32,
    bounds: [u32; 4],
}

// BallonsTranslator avoids model inference for a text block when the non-text
// pixels inside its balloon are nearly uniform, and otherwise sends an enlarged
// block crop to the inpainter:
// https://github.com/dmMaze/BallonsTranslator/blob/4bcc635c19f6c63a902872cf77b3d554e14ed1b7/ballontranslator/modules/inpaint/base.py#L168-L200
// Koharu uses the detected bubble polygons as those blocks. Uniform bubbles are
// filled first; only the remaining mask is split into bounded model crops.
fn inpaint_tiled(
    image: &DynamicImage,
    mask: &GrayImage,
    text_mask: &GrayImage,
    flat_fill_regions: &[FlatFillRegion],
    mut inference: impl FnMut(&DynamicImage, &GrayImage) -> Result<DynamicImage>,
) -> Result<DynamicImage> {
    ensure!(
        image.dimensions() == mask.dimensions() && mask.dimensions() == text_mask.dimensions(),
        "image and mask dimensions differ: image={:?}, mask={:?}, text_mask={:?}",
        image.dimensions(),
        mask.dimensions(),
        text_mask.dimensions()
    );

    let mut output = image.to_rgb8();
    let mut pending_mask = mask.clone();
    fill_uniform_regions(&mut output, &mut pending_mask, text_mask, flat_fill_regions);
    let (component_labels, tiles) = inpaint_tiles(&pending_mask);
    for tile in &tiles {
        let [left, top, right, bottom] = tile.crop;
        let crop_width = right - left;
        let crop_height = bottom - top;
        let crop_image =
            image::imageops::crop_imm(&output, left, top, crop_width, crop_height).to_image();
        let crop_mask = crop_tile_mask(&pending_mask, tile);

        let generated = inference(&DynamicImage::ImageRgb8(crop_image), &crop_mask)?;
        let generated = if generated.dimensions() == (crop_width, crop_height) {
            generated.to_rgb8()
        } else {
            generated
                .resize_exact(
                    crop_width,
                    crop_height,
                    image::imageops::FilterType::Lanczos3,
                )
                .to_rgb8()
        };
        composite_generated(&mut output, &component_labels, tile, &generated);
    }
    Ok(DynamicImage::ImageRgb8(output))
}

fn fill_uniform_regions(
    output: &mut RgbImage,
    pending_mask: &mut GrayImage,
    text_mask: &GrayImage,
    regions: &[FlatFillRegion],
) {
    for region in regions {
        let [left, top, right, bottom] = region.bounds;
        let mut targets = Vec::new();
        for y in top..bottom {
            for x in left..right {
                if pending_mask.get_pixel(x, y)[0] >= 127
                    && text_mask.get_pixel(x, y)[0] >= 127
                    && point_in_polygon((x as f32 + 0.5, y as f32 + 0.5), &region.polygon)
                {
                    targets.push((x, y));
                }
            }
        }
        if targets.is_empty() {
            continue;
        }
        let Some(color) = uniform_region_color(output, pending_mask, region) else {
            continue;
        };
        for (x, y) in targets {
            output.put_pixel(x, y, color);
            pending_mask.put_pixel(x, y, Luma([0]));
        }
    }
}

fn inpaint_tiles(mask: &GrayImage) -> (ImageBuffer<Luma<u32>, Vec<u32>>, Vec<InpaintTile>) {
    let (labels, components) = mask_components(mask);
    let mut bounded_tiles: Vec<InpaintTile> = Vec::new();
    let mut split_tiles = Vec::new();

    for component in components {
        let [left, top, right, bottom] = component.bounds;
        if right - left <= TILE_SIZE && bottom - top <= TILE_SIZE {
            let best = bounded_tiles
                .iter()
                .enumerate()
                .filter_map(|(index, tile)| {
                    let bounds = union_bounds(tile.core, component.bounds);
                    ((bounds[2] - bounds[0] <= TILE_SIZE) && (bounds[3] - bounds[1] <= TILE_SIZE))
                        .then_some((bounds_area(bounds) - bounds_area(tile.core), index, bounds))
                })
                .min_by_key(|(growth, index, _)| (*growth, *index));
            if let Some((_, index, bounds)) = best {
                let tile = &mut bounded_tiles[index];
                tile.components.push(component.label);
                tile.core = bounds;
                tile.crop = expand_bounds(bounds, mask.width(), mask.height());
            } else {
                bounded_tiles.push(InpaintTile {
                    components: vec![component.label],
                    core: component.bounds,
                    crop: expand_bounds(component.bounds, mask.width(), mask.height()),
                });
            }
            continue;
        }

        let mut core_top = top;
        while core_top < bottom {
            let core_bottom = core_top.saturating_add(TILE_SIZE).min(bottom);
            let mut core_left = left;
            while core_left < right {
                let core_right = core_left.saturating_add(TILE_SIZE).min(right);
                let core = [core_left, core_top, core_right, core_bottom];
                if let Some(owned_bounds) = component_bounds_in(&labels, component.label, core) {
                    split_tiles.push(InpaintTile {
                        components: vec![component.label],
                        core,
                        crop: expand_bounds(owned_bounds, mask.width(), mask.height()),
                    });
                }
                core_left = core_right;
            }
            core_top = core_bottom;
        }
    }

    bounded_tiles.append(&mut split_tiles);
    bounded_tiles.sort_by_key(|tile| (tile.core[1], tile.core[0], tile.core[3], tile.core[2]));
    (labels, bounded_tiles)
}

fn mask_components(mask: &GrayImage) -> (ImageBuffer<Luma<u32>, Vec<u32>>, Vec<MaskComponent>) {
    let binary = GrayImage::from_fn(mask.width(), mask.height(), |x, y| {
        Luma([(mask.get_pixel(x, y)[0] >= 127) as u8 * u8::MAX])
    });
    let labels = connected_components(&binary, Connectivity::Eight, Luma([0]));
    let mut bounds = Vec::<Option<[u32; 4]>>::new();
    for (x, y, pixel) in labels.enumerate_pixels() {
        let label = pixel[0] as usize;
        if label == 0 {
            continue;
        }
        if label >= bounds.len() {
            bounds.resize(label + 1, None);
        }
        if let Some(bounds) = &mut bounds[label] {
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + 1);
            bounds[3] = bounds[3].max(y + 1);
        } else {
            bounds[label] = Some([x, y, x + 1, y + 1]);
        }
    }
    let components = bounds
        .into_iter()
        .enumerate()
        .skip(1)
        .filter_map(|(label, bounds)| {
            bounds.map(|bounds| MaskComponent {
                label: label as u32,
                bounds,
            })
        })
        .collect();
    (labels, components)
}

fn union_bounds(left: [u32; 4], right: [u32; 4]) -> [u32; 4] {
    [
        left[0].min(right[0]),
        left[1].min(right[1]),
        left[2].max(right[2]),
        left[3].max(right[3]),
    ]
}

fn bounds_area([left, top, right, bottom]: [u32; 4]) -> u64 {
    u64::from(right - left) * u64::from(bottom - top)
}

fn expand_bounds([left, top, right, bottom]: [u32; 4], width: u32, height: u32) -> [u32; 4] {
    [
        left.saturating_sub(TILE_CONTEXT),
        top.saturating_sub(TILE_CONTEXT),
        right.saturating_add(TILE_CONTEXT).min(width),
        bottom.saturating_add(TILE_CONTEXT).min(height),
    ]
}

fn component_bounds_in(
    labels: &ImageBuffer<Luma<u32>, Vec<u32>>,
    label: u32,
    [region_left, region_top, region_right, region_bottom]: [u32; 4],
) -> Option<[u32; 4]> {
    let mut left = region_right;
    let mut top = region_bottom;
    let mut right = 0;
    let mut bottom = 0;
    for y in region_top..region_bottom {
        for x in region_left..region_right {
            if labels.get_pixel(x, y)[0] == label {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x + 1);
                bottom = bottom.max(y + 1);
            }
        }
    }
    (right > left && bottom > top).then_some([left, top, right, bottom])
}

fn crop_tile_mask(mask: &GrayImage, tile: &InpaintTile) -> GrayImage {
    let [left, top, right, bottom] = tile.crop;
    let mut crop = GrayImage::new(right - left, bottom - top);
    for y in top..bottom {
        for x in left..right {
            if mask.get_pixel(x, y)[0] >= 127 {
                crop.put_pixel(x - left, y - top, Luma([u8::MAX]));
            }
        }
    }
    crop
}

fn uniform_region_color(
    image: &RgbImage,
    mask: &GrayImage,
    region: &FlatFillRegion,
) -> Option<Rgb<u8>> {
    let [left, top, right, bottom] = region.bounds;
    let mut channels = [Vec::new(), Vec::new(), Vec::new()];
    for y in top..bottom {
        for x in left..right {
            let point = (x as f32 + 0.5, y as f32 + 0.5);
            if mask.get_pixel(x, y)[0] >= 127
                || !point_in_polygon(point, &region.polygon)
                || polygon_edge_distance_squared(point, &region.polygon)
                    < FLAT_FILL_EDGE_MARGIN * FLAT_FILL_EDGE_MARGIN
            {
                continue;
            }
            let pixel = image.get_pixel(x, y);
            for channel in 0..3 {
                channels[channel].push(pixel[channel]);
            }
        }
    }
    if channels[0].len() < UNIFORM_BACKGROUND_MIN_PIXELS {
        return None;
    }

    let medians = channels.each_mut().map(|values| median(values));
    let deviations = std::array::from_fn::<_, 3, _>(|channel| {
        standard_deviation(&channels[channel], f64::from(medians[channel]))
    });
    let mean_deviation = deviations.iter().sum::<f64>() / deviations.len() as f64;
    let channel_spread = (deviations
        .iter()
        .map(|deviation| (deviation - mean_deviation).powi(2))
        .sum::<f64>()
        / deviations.len() as f64)
        .sqrt();
    let threshold = if channel_spread > 1.0 { 7.0 } else { 10.0 };
    (deviations.iter().copied().fold(0.0, f64::max) < threshold).then_some(Rgb(medians))
}

fn point_in_polygon(point: (f32, f32), polygon: &[(f32, f32)]) -> bool {
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        if (current.1 > point.1) != (previous.1 > point.1) {
            let intersection_x = (previous.0 - current.0) * (point.1 - current.1)
                / (previous.1 - current.1)
                + current.0;
            if point.0 < intersection_x {
                inside = !inside;
            }
        }
        previous = current;
    }
    inside
}

fn polygon_edge_distance_squared(point: (f32, f32), polygon: &[(f32, f32)]) -> f32 {
    let mut minimum = f32::INFINITY;
    let mut start = polygon[polygon.len() - 1];
    for &end in polygon {
        let segment = (end.0 - start.0, end.1 - start.1);
        let length_squared = segment.0 * segment.0 + segment.1 * segment.1;
        let projection = if length_squared > 0.0 {
            ((point.0 - start.0) * segment.0 + (point.1 - start.1) * segment.1) / length_squared
        } else {
            0.0
        }
        .clamp(0.0, 1.0);
        let closest = (
            start.0 + segment.0 * projection,
            start.1 + segment.1 * projection,
        );
        let dx = point.0 - closest.0;
        let dy = point.1 - closest.1;
        minimum = minimum.min(dx * dx + dy * dy);
        start = end;
    }
    minimum
}

fn median(values: &mut [u8]) -> u8 {
    values.sort_unstable();
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        ((u16::from(values[middle - 1]) + u16::from(values[middle])) / 2) as u8
    } else {
        values[middle]
    }
}

fn standard_deviation(values: &[u8], center: f64) -> f64 {
    (values
        .iter()
        .map(|value| (f64::from(*value) - center).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt()
}

fn composite_generated(
    output: &mut RgbImage,
    labels: &ImageBuffer<Luma<u32>, Vec<u32>>,
    tile: &InpaintTile,
    generated: &RgbImage,
) {
    let [left, top, _, _] = tile.crop;
    let [core_left, core_top, core_right, core_bottom] = tile.core;
    for y in core_top..core_bottom {
        for x in core_left..core_right {
            if tile
                .components
                .binary_search(&labels.get_pixel(x, y)[0])
                .is_ok()
            {
                output.put_pixel(x, y, *generated.get_pixel(x - left, y - top));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn transient_inpainting_mask_replaces_persistent_page_masks() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let mut page = None;
        let source = DynamicImage::new_rgb8(8, 8);
        let persistent = DynamicImage::ImageLuma8(GrayImage::from_pixel(8, 8, Luma([255])));
        let encode = |image: &DynamicImage| {
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, ImageFormat::Png).unwrap();
            Arc::<[u8]>::from(bytes.into_inner())
        };
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(
                    koharu_scene::PageDraft::new("page", 8.0, 8.0),
                    koharu_scene::At::End,
                )?;
                for (role, image) in [("source", &source), ("text-mask", &persistent)] {
                    edit.set_asset(
                        id,
                        &AssetRole::new(role)?,
                        AssetInput::new(
                            encode(image),
                            "image/png",
                            AssetMetadata {
                                width: Some(8),
                                height: Some(8),
                                attributes: BTreeMap::new(),
                            },
                        ),
                    )?;
                }
                page = Some(id);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let page = page.unwrap();
        let mut transient = GrayImage::new(8, 8);
        transient.put_pixel(3, 4, Luma([255]));
        let input = StageInput::new(
            snapshot,
            page,
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            Some(crate::InpaintingMask {
                page,
                png: encode(&DynamicImage::ImageLuma8(transient)),
            }),
        );

        let prepared = prepare(&input).await.unwrap();
        assert_eq!(prepared.mask.get_pixel(3, 4), &Luma([255]));
        assert_eq!(prepared.mask.get_pixel(0, 0), &Luma([0]));
        assert!(prepared.text_mask.pixels().all(|pixel| pixel[0] == 0));
    }

    #[tokio::test]
    async fn automatic_inpainting_skips_committed_cleanup() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let mut page = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(
                    koharu_scene::PageDraft::new("page", 8.0, 8.0),
                    koharu_scene::At::End,
                )?;
                let cleanup = edit.add_entity(id, At::Start)?;
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
                    &AssetRole::new("source")?,
                    AssetInput::new(
                        Arc::<[u8]>::from([0]),
                        "image/png",
                        AssetMetadata {
                            width: Some(8),
                            height: Some(8),
                            attributes: BTreeMap::new(),
                        },
                    ),
                )?;
                page = Some(id);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let page = page.unwrap();
        let automatic = StageInput::new(
            snapshot.clone(),
            page,
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            None,
        );
        let manual = StageInput::new(
            snapshot,
            page,
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            Some(crate::InpaintingMask {
                page,
                png: Arc::<[u8]>::from([]),
            }),
        );
        let processor = Processor::new(InpaintingModel::LaMa {}, koharu_ml::Device::cpu()).unwrap();

        assert!(processor.skip(&automatic).unwrap());
        assert!(!processor.skip(&manual).unwrap());
    }

    fn rectangle_region([left, top, right, bottom]: [u32; 4]) -> FlatFillRegion {
        FlatFillRegion {
            bounds: [left, top, right, bottom],
            polygon: vec![
                (left as f32, top as f32),
                (right as f32, top as f32),
                (right as f32, bottom as f32),
                (left as f32, bottom as f32),
            ],
        }
    }

    #[test]
    fn uniform_mask_background_is_filled_without_inference() {
        let mut image = RgbImage::from_pixel(96, 96, Rgb([240, 241, 242]));
        let mut mask = GrayImage::new(96, 96);
        for y in 40..56 {
            for x in 32..64 {
                image.put_pixel(x, y, Rgb([20, 20, 20]));
                mask.put_pixel(x, y, Luma([u8::MAX]));
            }
        }
        let mut calls = 0;

        let output = inpaint_tiled(
            &DynamicImage::ImageRgb8(image),
            &mask,
            &mask,
            &[rectangle_region([0, 0, 96, 96])],
            |_, _| {
                calls += 1;
                Ok(DynamicImage::new_rgb8(1, 1))
            },
        )
        .unwrap()
        .to_rgb8();

        assert_eq!(calls, 0);
        assert_eq!(output.get_pixel(48, 48), &Rgb([240, 241, 242]));
    }

    #[test]
    fn uniform_bubbles_are_filled_independently_inside_one_textured_tile() {
        let mut image = RgbImage::new(200, 100);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = if (x + y).is_multiple_of(2) {
                Rgb([20, 80, 140])
            } else {
                Rgb([220, 80, 120])
            };
        }
        for y in 10..90 {
            for x in 10..90 {
                image.put_pixel(x, y, Rgb([245, 245, 245]));
            }
            for x in 110..190 {
                image.put_pixel(x, y, Rgb([250, 240, 220]));
            }
        }
        let mut mask = GrayImage::new(200, 100);
        for y in 40..60 {
            for x in 30..60 {
                image.put_pixel(x, y, Rgb([10, 10, 10]));
                mask.put_pixel(x, y, Luma([u8::MAX]));
            }
            for x in 135..165 {
                image.put_pixel(x, y, Rgb([10, 10, 10]));
                mask.put_pixel(x, y, Luma([u8::MAX]));
            }
        }
        let mut calls = 0;

        let output = inpaint_tiled(
            &DynamicImage::ImageRgb8(image),
            &mask,
            &mask,
            &[
                rectangle_region([10, 10, 90, 90]),
                rectangle_region([110, 10, 190, 90]),
            ],
            |_, _| {
                calls += 1;
                Ok(DynamicImage::new_rgb8(1, 1))
            },
        )
        .unwrap()
        .to_rgb8();

        assert_eq!(calls, 0);
        assert_eq!(output.get_pixel(45, 50), &Rgb([245, 245, 245]));
        assert_eq!(output.get_pixel(150, 50), &Rgb([250, 240, 220]));
    }

    #[test]
    fn connected_mask_crossing_both_page_grid_axes_is_inferred_once() {
        let image = RgbImage::from_pixel(900, 900, Rgb([20, 40, 60]));
        let mut mask = GrayImage::new(900, 900);
        for y in 500..525 {
            for x in 500..525 {
                mask.put_pixel(x, y, Luma([u8::MAX]));
            }
        }
        let mut calls = 0;

        let output = inpaint_tiled(
            &DynamicImage::ImageRgb8(image),
            &mask,
            &mask,
            &[],
            |tile, tile_mask| {
                calls += 1;
                assert_eq!(
                    tile_mask.pixels().filter(|pixel| pixel[0] >= 127).count(),
                    25 * 25
                );
                Ok(DynamicImage::ImageRgb8(RgbImage::from_pixel(
                    tile.width(),
                    tile.height(),
                    Rgb([1, 2, 3]),
                )))
            },
        )
        .unwrap()
        .to_rgb8();

        assert_eq!(calls, 1);
        assert_eq!(output.get_pixel(500, 500), &Rgb([1, 2, 3]));
        assert_eq!(output.get_pixel(524, 524), &Rgb([1, 2, 3]));
        assert_eq!(output.get_pixel(499, 499), &Rgb([20, 40, 60]));
    }

    #[test]
    fn oversized_component_tiles_mask_their_continuation_context() {
        let mut mask = GrayImage::new(1000, 300);
        for y in 100..120 {
            for x in 50..850 {
                mask.put_pixel(x, y, Luma([u8::MAX]));
            }
        }

        let (labels, tiles) = inpaint_tiles(&mask);

        assert_eq!(tiles.len(), 2);
        for tile in &tiles {
            assert!(tile.crop[2] - tile.crop[0] <= TILE_SIZE + TILE_CONTEXT * 2);
            assert!(tile.crop[3] - tile.crop[1] <= TILE_SIZE + TILE_CONTEXT * 2);
            let label = tile.components[0];
            let [left, top, right, bottom] = tile.core;
            let mut owned = 0;
            for y in top..bottom {
                for x in left..right {
                    if labels.get_pixel(x, y)[0] == label {
                        owned += 1;
                    }
                }
            }
            let masked = crop_tile_mask(&mask, tile)
                .pixels()
                .filter(|pixel| pixel[0] >= 127)
                .count();
            assert!(masked > owned);
        }
    }

    #[test]
    fn textured_mask_regions_are_inferred_as_bounded_tiles() {
        let mut image = RgbImage::new(1200, 700);
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = if (x + y).is_multiple_of(2) {
                Rgb([0, 40, 80])
            } else {
                Rgb([255, 215, 175])
            };
        }
        let original = image.clone();
        let mut mask = GrayImage::new(1200, 700);
        mask.put_pixel(10, 10, Luma([u8::MAX]));
        mask.put_pixel(1100, 600, Luma([u8::MAX]));
        let mut calls = 0;

        let output = inpaint_tiled(
            &DynamicImage::ImageRgb8(image),
            &mask,
            &mask,
            &[],
            |tile, _| {
                calls += 1;
                assert!(tile.width() <= TILE_CONTEXT * 2 + 1);
                assert!(tile.height() <= TILE_CONTEXT * 2 + 1);
                Ok(DynamicImage::ImageRgb8(RgbImage::from_pixel(
                    tile.width(),
                    tile.height(),
                    Rgb([1, 2, 3]),
                )))
            },
        )
        .unwrap()
        .to_rgb8();

        assert_eq!(calls, 2);
        assert_eq!(output.get_pixel(10, 10), &Rgb([1, 2, 3]));
        assert_eq!(output.get_pixel(1100, 600), &Rgb([1, 2, 3]));
        assert_eq!(output.get_pixel(500, 300), original.get_pixel(500, 300));
    }

    #[test]
    fn sfx_adaptive_dilation_expands_mask_perimeter() {
        let mut mask = GrayImage::new(100, 100);
        for y in 45..55 {
            for x in 45..55 {
                mask.put_pixel(x, y, Luma([u8::MAX]));
            }
        }
        let initial_count = mask.iter().filter(|&&p| p > 0).count();
        let dilated = dilate(&mask, Norm::L2, 3);
        let dilated_count = dilated.iter().filter(|&&p| p > 0).count();
        assert!(dilated_count > initial_count);
    }

    #[test]
    fn fill_polygon_on_mask_rasterizes_polygon_points() {
        let mut mask = GrayImage::new(100, 100);
        let points = vec![
            koharu_scene::Point { x: 20.0, y: 20.0 },
            koharu_scene::Point { x: 80.0, y: 20.0 },
            koharu_scene::Point { x: 80.0, y: 80.0 },
            koharu_scene::Point { x: 20.0, y: 80.0 },
        ];
        super::fill_polygon_on_mask(&mut mask, &points);
        assert_eq!(mask.get_pixel(50, 50), &Luma([255]));
        assert_eq!(mask.get_pixel(10, 10), &Luma([0]));
        assert_eq!(mask.get_pixel(90, 90), &Luma([0]));
    }

    #[tokio::test]
    async fn manual_text_layers_are_included_in_inpainting_mask_even_without_text_mask_asset() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let source = DynamicImage::new_rgb8(100, 100);
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let png = Arc::<[u8]>::from(bytes.into_inner());

        let mut page_id = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(
                    koharu_scene::PageDraft::new("page", 100.0, 100.0),
                    koharu_scene::At::End,
                )?;
                edit.set_asset(
                    id,
                    &AssetRole::new("source")?,
                    AssetInput::new(
                        png,
                        "image/png",
                        AssetMetadata {
                            width: Some(100),
                            height: Some(100),
                            attributes: BTreeMap::new(),
                        },
                    ),
                )?;
                let content = edit.add_text_content(id, At::End)?;
                let layer = edit.add_text_layer(
                    id,
                    At::End,
                    content,
                    &TextLayout {
                        origin: Origin::User,
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set(
                    layer,
                    &Geometry {
                        origin: Origin::User,
                        points: vec![
                            koharu_scene::Point { x: 30.0, y: 30.0 },
                            koharu_scene::Point { x: 70.0, y: 30.0 },
                            koharu_scene::Point { x: 70.0, y: 70.0 },
                            koharu_scene::Point { x: 30.0, y: 70.0 },
                        ],
                    },
                )?;
                page_id = Some(id);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let page = page_id.unwrap();
        let input = StageInput::new(
            snapshot,
            page,
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            None,
        );
        let prepared = prepare(&input).await.unwrap();
        assert_eq!(prepared.mask.get_pixel(50, 50), &Luma([255]));
        assert_eq!(prepared.text_mask.get_pixel(50, 50), &Luma([255]));
        assert_eq!(prepared.mask.get_pixel(10, 10), &Luma([0]));
    }

    #[tokio::test]
    async fn region_scoped_sfx_dilation_does_not_bleed_outside_region_bounds() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let source = DynamicImage::new_rgb8(1024, 1024);
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let png = Arc::<[u8]>::from(bytes.into_inner());

        let mut page_id = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(
                    koharu_scene::PageDraft::new("page", 1024.0, 1024.0),
                    koharu_scene::At::End,
                )?;
                edit.set_asset(
                    id,
                    &AssetRole::new("source")?,
                    AssetInput::new(
                        png,
                        "image/png",
                        AssetMetadata {
                            width: Some(1024),
                            height: Some(1024),
                            attributes: BTreeMap::new(),
                        },
                    ),
                )?;
                let content = edit.add_text_content(id, At::End)?;
                edit.set(
                    content,
                    &koharu_scene::TextRole {
                        origin: Origin::User,
                        role: "dev.koharu.text.sfx".to_owned(),
                    },
                )?;
                let layer = edit.add_text_layer(
                    id,
                    At::End,
                    content,
                    &TextLayout {
                        origin: Origin::User,
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set(
                    layer,
                    &Geometry {
                        origin: Origin::User,
                        points: vec![
                            koharu_scene::Point { x: 102.0, y: 102.0 },
                            koharu_scene::Point { x: 198.0, y: 102.0 },
                            koharu_scene::Point { x: 198.0, y: 198.0 },
                            koharu_scene::Point { x: 102.0, y: 198.0 },
                        ],
                    },
                )?;
                page_id = Some(id);
                Ok(())
            })
            .unwrap();

        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let page = page_id.unwrap();
        let bounds = crate::Bounds {
            x: 100.0,
            y: 100.0,
            width: 100.0,
            height: 100.0,
        };
        let input = StageInput::new(
            snapshot,
            page,
            None,
            Some(bounds),
            Arc::new(crate::ImageCache::default()),
            None,
        );

        let prepared = prepare(&input).await.unwrap();

        assert_eq!(prepared.mask.get_pixel(150, 150), &Luma([255]));

        for (x, y, pixel) in prepared.mask.enumerate_pixels() {
            if x < 100 || x >= 200 || y < 100 || y >= 200 {
                assert_eq!(
                    pixel[0], 0,
                    "Mask leaked outside bounds at ({x}, {y}): value={}",
                    pixel[0]
                );
            }
        }
        for (x, y, pixel) in prepared.text_mask.enumerate_pixels() {
            if x < 100 || x >= 200 || y < 100 || y >= 200 {
                assert_eq!(
                    pixel[0], 0,
                    "Text mask leaked outside bounds at ({x}, {y}): value={}",
                    pixel[0]
                );
            }
        }
    }

    #[tokio::test]
    async fn sfx_dilation_does_not_dilate_dialogue_text_layers() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let source = DynamicImage::new_rgb8(1024, 1024);
        let mut bytes = Cursor::new(Vec::new());
        source.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let png = Arc::<[u8]>::from(bytes.into_inner());

        let mut page_id = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let id = edit.add_page(
                    koharu_scene::PageDraft::new("page", 1024.0, 1024.0),
                    koharu_scene::At::End,
                )?;
                edit.set_asset(
                    id,
                    &AssetRole::new("source")?,
                    AssetInput::new(
                        png,
                        "image/png",
                        AssetMetadata {
                            width: Some(1024),
                            height: Some(1024),
                            attributes: BTreeMap::new(),
                        },
                    ),
                )?;

                let dialogue_content = edit.add_text_content(id, At::End)?;
                edit.set(
                    dialogue_content,
                    &koharu_scene::TextRole {
                        origin: Origin::User,
                        role: "dev.koharu.text.dialogue".to_owned(),
                    },
                )?;
                let dialogue_layer = edit.add_text_layer(
                    id,
                    At::End,
                    dialogue_content,
                    &TextLayout {
                        origin: Origin::User,
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set(
                    dialogue_layer,
                    &Geometry {
                        origin: Origin::User,
                        points: vec![
                            koharu_scene::Point { x: 100.0, y: 100.0 },
                            koharu_scene::Point { x: 200.0, y: 100.0 },
                            koharu_scene::Point { x: 200.0, y: 200.0 },
                            koharu_scene::Point { x: 100.0, y: 200.0 },
                        ],
                    },
                )?;

                let sfx_content = edit.add_text_content(id, At::End)?;
                edit.set(
                    sfx_content,
                    &koharu_scene::TextRole {
                        origin: Origin::User,
                        role: "dev.koharu.text.sfx".to_owned(),
                    },
                )?;
                let sfx_layer = edit.add_text_layer(
                    id,
                    At::End,
                    sfx_content,
                    &TextLayout {
                        origin: Origin::User,
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set(
                    sfx_layer,
                    &Geometry {
                        origin: Origin::User,
                        points: vec![
                            koharu_scene::Point { x: 500.0, y: 500.0 },
                            koharu_scene::Point { x: 600.0, y: 500.0 },
                            koharu_scene::Point { x: 600.0, y: 600.0 },
                            koharu_scene::Point { x: 500.0, y: 600.0 },
                        ],
                    },
                )?;

                page_id = Some(id);
                Ok(())
            })
            .unwrap();

        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let page = page_id.unwrap();
        let input = StageInput::new(
            snapshot,
            page,
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            None,
        );

        let prepared = prepare(&input).await.unwrap();

        assert_eq!(prepared.mask.get_pixel(98, 150), &Luma([0]));
        assert_eq!(prepared.mask.get_pixel(99, 150), &Luma([0]));
        assert_eq!(prepared.mask.get_pixel(100, 150), &Luma([255]));

        assert_eq!(prepared.mask.get_pixel(498, 550), &Luma([255]));
        assert_eq!(prepared.mask.get_pixel(550, 550), &Luma([255]));
    }

    #[test]
    fn scanline_rasterization_matches_convex_and_concave_polygons() {
        let mut mask = GrayImage::new(100, 100);

        let points = vec![
            koharu_scene::Point { x: 20.0, y: 20.0 },
            koharu_scene::Point { x: 80.0, y: 20.0 },
            koharu_scene::Point { x: 80.0, y: 80.0 },
            koharu_scene::Point { x: 60.0, y: 80.0 },
            koharu_scene::Point { x: 60.0, y: 40.0 },
            koharu_scene::Point { x: 20.0, y: 40.0 },
        ];
        super::fill_polygon_on_mask(&mut mask, &points);

        assert_eq!(mask.get_pixel(50, 30), &Luma([255]));
        assert_eq!(mask.get_pixel(70, 60), &Luma([255]));
        assert_eq!(mask.get_pixel(40, 60), &Luma([0]));
        assert_eq!(mask.get_pixel(10, 10), &Luma([0]));
        assert_eq!(mask.get_pixel(90, 90), &Luma([0]));
    }

    #[test]
    fn scanline_rasterization_bitwise_parity_with_point_in_polygon() {
        let width = 300;
        let height = 300;
        let mut scanline_mask = GrayImage::new(width, height);
        let mut raycast_mask = GrayImage::new(width, height);

        let points = vec![
            koharu_scene::Point { x: 100.0, y: 50.0 },
            koharu_scene::Point { x: 150.0, y: 55.0 },
            koharu_scene::Point { x: 200.0, y: 70.0 },
            koharu_scene::Point { x: 230.0, y: 110.0 },
            koharu_scene::Point { x: 235.0, y: 160.0 },
            koharu_scene::Point { x: 210.0, y: 210.0 },
            koharu_scene::Point { x: 160.0, y: 240.0 },
            koharu_scene::Point { x: 120.0, y: 235.0 },
            koharu_scene::Point { x: 80.0, y: 270.0 },
            koharu_scene::Point { x: 90.0, y: 220.0 },
            koharu_scene::Point { x: 65.0, y: 170.0 },
            koharu_scene::Point { x: 70.0, y: 100.0 },
        ];

        super::fill_polygon_on_mask(&mut scanline_mask, &points);

        let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
        let (mut max_x, mut max_y) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in &points {
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }
        let start_x = min_x.floor().clamp(0.0, width as f64) as u32;
        let end_x = max_x.ceil().clamp(0.0, width as f64) as u32;
        let start_y = min_y.floor().clamp(0.0, height as f64) as u32;
        let end_y = max_y.ceil().clamp(0.0, height as f64) as u32;

        for y in start_y..end_y {
            let py = y as f64 + 0.5;
            for x in start_x..end_x {
                let px = x as f64 + 0.5;
                let mut inside = false;
                let mut j = points.len() - 1;
                for i in 0..points.len() {
                    let pi = &points[i];
                    let pj = &points[j];
                    if ((pi.y > py) != (pj.y > py))
                        && (px < (pj.x - pi.x) * (py - pi.y) / (pj.y - pi.y) + pi.x)
                    {
                        inside = !inside;
                    }
                    j = i;
                }
                if inside {
                    raycast_mask.put_pixel(x, y, Luma([255]));
                }
            }
        }

        assert_eq!(scanline_mask.as_raw(), raycast_mask.as_raw());
    }

    #[tokio::test]
    async fn refine_floating_text_halos_updates_generated_floating_text_and_skips_user_typography() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let det_gen = generation("dev.koharu.pipeline.detection", "rfdetr").unwrap();
        let inp_gen = generation(PRODUCER, "lama").unwrap();

        let mut page_id = None;
        let mut floating_layer_id = None;
        let mut user_layer_id = None;
        let mut bubble_layer_id = None;

        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(
                    koharu_scene::PageDraft::new("page", 128.0, 128.0),
                    At::End,
                )?;
                let floating_region = edit.add_analysis_region::<TextRegion>(
                    page,
                    At::End,
                    &Geometry::rectangle(16.0, 16.0, 32.0, 20.0),
                    None,
                )?;
                let floating_content = edit.add_text_content(page, At::End)?;
                let floating_layer = edit.add_text_layer(
                    page,
                    At::End,
                    floating_content,
                    &TextLayout {
                        origin: Origin::Generated(det_gen.clone()),
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.relate::<koharu_scene::FitsTo>(floating_layer, floating_region)?;
                edit.set(
                    floating_layer,
                    &Typography {
                        origin: Origin::Generated(det_gen.clone()),
                        preferred_font: None,
                        font_weight: None,
                        font_style: None,
                        size: Some(24.0),
                        auto_fit: true,
                        color: Some([0, 0, 0, 255]),
                        stroke_color: None,
                        stroke_width: None,
                        alignment: None,
                        writing_mode: Some(WritingMode::Horizontal),
                        extensions: BTreeMap::new(),
                    },
                )?;

                let user_region = edit.add_analysis_region::<TextRegion>(
                    page,
                    At::End,
                    &Geometry::rectangle(16.0, 56.0, 32.0, 20.0),
                    None,
                )?;
                let user_content = edit.add_text_content(page, At::End)?;
                let user_layer = edit.add_text_layer(
                    page,
                    At::End,
                    user_content,
                    &TextLayout {
                        origin: Origin::User,
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.relate::<koharu_scene::FitsTo>(user_layer, user_region)?;
                edit.set(
                    user_layer,
                    &Typography {
                        origin: Origin::User,
                        preferred_font: Some("CustomFont".to_owned()),
                        font_weight: None,
                        font_style: None,
                        size: Some(22.0),
                        auto_fit: false,
                        color: Some([10, 10, 10, 255]),
                        stroke_color: None,
                        stroke_width: None,
                        alignment: None,
                        writing_mode: Some(WritingMode::Horizontal),
                        extensions: BTreeMap::new(),
                    },
                )?;

                let bubble_region = edit.add_analysis_region::<BubbleRegion>(
                    page,
                    At::End,
                    &Geometry::rectangle(72.0, 16.0, 44.0, 44.0),
                    None,
                )?;
                let bubble_content = edit.add_text_content(page, At::End)?;
                let bubble_layer = edit.add_text_layer(
                    page,
                    At::End,
                    bubble_content,
                    &TextLayout {
                        origin: Origin::Generated(det_gen.clone()),
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.relate::<FlowsIn>(bubble_layer, bubble_region)?;
                edit.set(
                    bubble_layer,
                    &Typography {
                        origin: Origin::Generated(det_gen.clone()),
                        preferred_font: None,
                        font_weight: None,
                        font_style: None,
                        size: Some(20.0),
                        auto_fit: true,
                        color: Some([0, 0, 0, 255]),
                        stroke_color: None,
                        stroke_width: None,
                        alignment: None,
                        writing_mode: Some(WritingMode::Horizontal),
                        extensions: BTreeMap::new(),
                    },
                )?;

                page_id = Some(page);
                floating_layer_id = Some(floating_layer);
                user_layer_id = Some(user_layer);
                bubble_layer_id = Some(bubble_layer);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(setup).await.unwrap().snapshot;
        let page = page_id.unwrap();

        let mut inpainted = RgbaImage::new(128, 128);
        for (x, y, pixel) in inpainted.enumerate_pixels_mut() {
            *pixel = if ((x / 2) + (y / 2)).is_multiple_of(2) {
                Rgba([12, 12, 12, 255])
            } else {
                Rgba([244, 244, 244, 255])
            };
        }

        let refine_patch = snapshot
            .patch(|edit| {
                refine_floating_text_halos(&snapshot, page, &inpainted, edit, &inp_gen)
            })
            .unwrap();
        let refined = session.commit(refine_patch).await.unwrap().snapshot;

        let floating_typo = refined
            .component::<Typography>(floating_layer_id.unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(floating_typo.origin, Origin::Generated(det_gen.clone()));
        assert_eq!(floating_typo.stroke_color, Some([255, 255, 255, 255]));
        assert!(floating_typo.stroke_width.is_some_and(|w| w >= 2.0));
        assert_eq!(
            floating_typo.outer_stroke().map(|(c, _)| c),
            Some([0, 0, 0, 255])
        );

        let user_typo = refined
            .component::<Typography>(user_layer_id.unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(user_typo.origin, Origin::User);
        assert_eq!(user_typo.stroke_color, None);
        assert_eq!(user_typo.stroke_width, None);
        assert_eq!(user_typo.outer_stroke(), None);

        let bubble_typo = refined
            .component::<Typography>(bubble_layer_id.unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(bubble_typo.stroke_color, None);
        assert_eq!(bubble_typo.stroke_width, None);
        assert_eq!(bubble_typo.outer_stroke(), None);
    }
}
