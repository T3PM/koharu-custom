use std::sync::{Arc, Mutex};

use super::{StageInput, StageProcessor, finish, generation};
use crate::{ModelCell, OcrModel, scope::geometry_extents};
use anyhow::{Context as _, Result, anyhow, bail};
use async_trait::async_trait;
use image::DynamicImage;
use koharu_ml::{
    baberu_ocr::BaberuOcr, hayai_ocr::HayaiOcr, manga_ocr::MangaOcr,
    paddle_ocr_vl::PaddleOCRVLTask, paddle_ocr_vl_quantized::PaddleOCRVLQuantized,
};
use koharu_scene::{
    Authored, EntityId, Geometry, LanguageTag, OcrAnalysis, Origin, Presents, RecognizedFrom,
    Region, RegionSpec, SourceText, TextDirection, TextLayout, TextRegion,
};

const PRODUCER: &str = "dev.koharu.pipeline.ocr";

pub(super) struct Processor {
    config: OcrModel,
    device: koharu_ml::Device,
    model: ModelCell<Model>,
}

impl Processor {
    pub(super) fn new(config: OcrModel, device: koharu_ml::Device) -> Self {
        Self {
            config,
            device,
            model: ModelCell::new(),
        }
    }
}

#[async_trait]
impl StageProcessor for Processor {
    fn model(&self) -> &'static str {
        match self.config {
            OcrModel::MangaOcr => "manga-ocr",
            OcrModel::BaberuOcr => "baberu-ocr",
            OcrModel::HayaiOcr => "hayai-ocr",
            OcrModel::PaddleOcrVl1_6 => "paddleocr-vl-1.6",
        }
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
            .ok_or_else(|| anyhow!("OCR model is not loaded"))?
            .run(input)
            .await
    }
}

enum Model {
    Manga(Arc<Mutex<MangaOcr>>),
    Baberu(Arc<Mutex<BaberuOcr>>),
    Hayai(Arc<Mutex<HayaiOcr>>),
    Paddle(Arc<Mutex<PaddleOCRVLQuantized>>),
}

impl Model {
    async fn load(device: koharu_ml::Device, config: &OcrModel) -> Result<Self> {
        match config {
            OcrModel::MangaOcr => Ok(Self::Manga(Arc::new(Mutex::new(
                MangaOcr::load(device).await?,
            )))),
            OcrModel::BaberuOcr => Ok(Self::Baberu(Arc::new(Mutex::new(
                BaberuOcr::load(device).await?,
            )))),
            OcrModel::HayaiOcr => Ok(Self::Hayai(Arc::new(Mutex::new(
                HayaiOcr::load(device).await?,
            )))),
            OcrModel::PaddleOcrVl1_6 => Ok(Self::Paddle(Arc::new(Mutex::new(
                PaddleOCRVLQuantized::load(device).await?,
            )))),
        }
    }

    async fn run(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        let page = input.page;
        let model_name = match self {
            Self::Manga(_) => "manga-ocr",
            Self::Baberu(_) => "baberu-ocr",
            Self::Hayai(_) => "hayai-ocr",
            Self::Paddle(_) => "paddleocr-vl-1.6",
        };
        let targets = collect_ocr_targets(&input).await?;

        let results = match self {
            Self::Manga(model) => {
                infer_text(model.clone(), targets, |model, image| {
                    model.inference(image)
                })
                .await?
            }
            Self::Baberu(model) => {
                infer_text(model.clone(), targets, |model, image| {
                    model.inference(image)
                })
                .await?
            }
            Self::Hayai(model) => {
                infer_text(model.clone(), targets, |model, image| {
                    model.inference(image)
                })
                .await?
            }
            Self::Paddle(model) => {
                infer_text(model.clone(), targets, |model, image| {
                    Ok(model.inference(image, PaddleOCRVLTask::Ocr)?.text)
                })
                .await?
            }
        };

        let generation = generation(PRODUCER, model_name)?;
        let mut edit = input.scene.edit_as(generation.clone());
        edit.observe_assets(page)?;
        for result in &results {
            if input.scene.component::<Region>(result.region)?.is_some() {
                edit.observe::<Region>(result.region)?;
            }
            if input.scene.component::<Geometry>(result.region)?.is_some() {
                edit.observe::<Geometry>(result.region)?;
            }
            if input.scene.component::<OcrAnalysis>(result.region)?.is_some() {
                edit.observe::<OcrAnalysis>(result.region)?;
            }
            edit.observe::<SourceText>(result.content)?;
        }
        for result in results {
            let language = result
                .previous
                .and_then(|value| value.language)
                .or_else(|| LanguageTag::new("ja-JP").ok());
            edit.set(
                result.content,
                &SourceText {
                    text: Authored::generated(result.text, generation.clone()),
                    language,
                },
            )?;
            let (min_x, min_y, max_x, max_y) = geometry_extents(&result.geometry)
                .ok_or_else(|| anyhow!("text region {} has empty geometry", result.region))?;
            edit.set(
                result.region,
                &OcrAnalysis {
                    origin: Origin::Generated(generation.clone()),
                    direction: if max_y - min_y >= (max_x - min_x) * 1.15 {
                        TextDirection::Vertical
                    } else {
                        TextDirection::Horizontal
                    },
                    confidence: None,
                    line_boundaries: Vec::new(),
                },
            )?;
        }
        finish(edit)
    }
}

struct OcrTarget {
    content: EntityId,
    region: EntityId,
    geometry: Geometry,
    previous: Option<SourceText>,
    image: DynamicImage,
}

struct OcrResult {
    content: EntityId,
    region: EntityId,
    geometry: Geometry,
    previous: Option<SourceText>,
    text: String,
}

async fn infer_text<M: Send + 'static>(
    model: Arc<Mutex<M>>,
    targets: Vec<OcrTarget>,
    inference: impl Fn(&M, &DynamicImage) -> Result<String> + Send + Sync + 'static,
) -> Result<Vec<OcrResult>> {
    tokio_rayon::spawn(move || {
        let model = model
            .lock()
            .map_err(|_| anyhow!("OCR model lock is poisoned"))?;
        targets
            .into_iter()
            .map(|target| {
                Ok(OcrResult {
                    content: target.content,
                    region: target.region,
                    geometry: target.geometry,
                    previous: target.previous,
                    text: normalize_ocr_text(inference(&model, &target.image)?),
                })
            })
            .collect()
    })
    .await
}

// Manga OCR can emit replacement-box glyphs for an isolated Japanese ellipsis.
// Normalize only an all-placeholder sequence so ordinary OCR output is preserved.
fn normalize_ocr_text(text: String) -> String {
    let visible = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<Vec<_>>();
    if visible.len() >= 2
        && visible
            .iter()
            .all(|character| matches!(character, '☐' | '□' | '▢' | '▣' | '�'))
    {
        "…".to_owned()
    } else {
        text
    }
}

async fn collect_ocr_targets(input: &StageInput) -> Result<Vec<OcrTarget>> {
    let page = input.page;
    let mut targets = Vec::new();
    let source = input
        .images
        .get(&input.scene, page, "source")
        .await?
        .ok_or_else(|| anyhow!("page {page} has no source image"))?;
    for entity in input.scene.descendants(page)? {
        let region = entity.id();
        if !input.contains_entity(region)? {
            continue;
        }
        let is_text_region = input
            .scene
            .component::<Region>(region)?
            .is_some_and(|value| value.kind == TextRegion::kind());
        if !is_text_region {
            continue;
        }
        let geometry = input
            .scene
            .component::<Geometry>(region)?
            .ok_or_else(|| anyhow!("text region {region} has no geometry"))?;
        let crop = crop(&source, &geometry)
            .with_context(|| format!("text region {region} is outside its source image"))?;
        for relation in input.scene.relations_to_as::<RecognizedFrom>(region) {
            let content = relation.value().source;
            let previous = input.scene.component::<SourceText>(content)?;
            if previous
                .as_ref()
                .is_some_and(|value| matches!(value.text.origin, Origin::User))
            {
                continue;
            }
            targets.push(OcrTarget {
                content,
                region,
                geometry: geometry.clone(),
                previous,
                image: crop.clone(),
            });
        }
    }

    let mut recognized_contents = targets
        .iter()
        .map(|t| t.content)
        .collect::<std::collections::HashSet<_>>();
    for entity in input.scene.descendants(page)? {
        let layer_id = entity.id();
        if !input.contains_entity(layer_id)? {
            continue;
        }
        if entity.component::<TextLayout>()?.is_none() {
            continue;
        }
        let Some(geometry) = crate::scope::resolve_geometry(&input.scene, layer_id)? else {
            continue;
        };
        for relation in input.scene.relations_from_as::<Presents>(layer_id) {
            let content = relation.value().target;
            if !recognized_contents.insert(content) {
                continue;
            }
            let previous = input.scene.component::<SourceText>(content)?;
            if previous.as_ref().is_some_and(|val| {
                matches!(val.text.origin, Origin::User) && !val.text.value.trim().is_empty()
            }) {
                continue;
            }
            let Ok(crop) = crop(&source, &geometry) else {
                continue;
            };
            targets.push(OcrTarget {
                content,
                region: layer_id,
                geometry: geometry.clone(),
                previous,
                image: crop,
            });
        }
    }
    Ok(targets)
}

fn crop(source: &DynamicImage, geometry: &Geometry) -> Result<DynamicImage> {
    let (min_x, min_y, max_x, max_y) =
        geometry_extents(geometry).ok_or_else(|| anyhow!("geometry is empty"))?;
    let x = min_x.floor().max(0.0) as u32;
    let y = min_y.floor().max(0.0) as u32;
    let right = max_x.ceil().max(0.0).min(f64::from(source.width())) as u32;
    let bottom = max_y.ceil().max(0.0).min(f64::from(source.height())) as u32;
    if right <= x || bottom <= y {
        bail!("geometry does not overlap the image");
    }
    Ok(source.crop_imm(x, y, right - x, bottom - y))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use super::*;

    #[test]
    fn repeated_placeholder_glyphs_are_an_ellipsis() {
        assert_eq!(normalize_ocr_text("☐ ☐ ☐".to_owned()), "…");
        assert_eq!(normalize_ocr_text("□\n□".to_owned()), "…");
    }

    #[test]
    fn ordinary_text_and_single_boxes_are_unchanged() {
        assert_eq!(normalize_ocr_text("待って…".to_owned()), "待って…");
        assert_eq!(normalize_ocr_text("☐".to_owned()), "☐");
    }

    #[tokio::test]
    async fn manual_text_layers_without_text_region_are_targeted_for_ocr() {
        let mut session = koharu_scene::Session::memory().await.unwrap();
        let source = image::DynamicImage::new_rgb8(64, 64);
        let mut bytes = std::io::Cursor::new(Vec::new());
        source.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        let png = Arc::<[u8]>::from(bytes.into_inner());

        let mut page_id = None;
        let mut content_id = None;
        let mut layer_id = None;

        let patch = session
            .snapshot()
            .patch(|edit| {
                let p = edit.add_page(
                    koharu_scene::PageDraft::new("page", 64.0, 64.0),
                    koharu_scene::At::End,
                )?;
                edit.set_asset(
                    p,
                    &koharu_scene::AssetRole::new("source")?,
                    koharu_scene::AssetInput::new(
                        png,
                        "image/png",
                        koharu_scene::AssetMetadata {
                            width: Some(64),
                            height: Some(64),
                            attributes: std::collections::BTreeMap::new(),
                        },
                    ),
                )?;
                let c = edit.add_text_content(p, koharu_scene::At::End)?;
                let l = edit.add_text_layer(
                    p,
                    koharu_scene::At::End,
                    c,
                    &koharu_scene::TextLayout {
                        origin: koharu_scene::Origin::User,
                        kind: koharu_scene::TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set(
                    l,
                    &koharu_scene::Geometry {
                        origin: koharu_scene::Origin::User,
                        points: vec![
                            koharu_scene::Point { x: 10.0, y: 10.0 },
                            koharu_scene::Point { x: 30.0, y: 10.0 },
                            koharu_scene::Point { x: 30.0, y: 30.0 },
                            koharu_scene::Point { x: 10.0, y: 30.0 },
                        ],
                    },
                )?;
                page_id = Some(p);
                content_id = Some(c);
                layer_id = Some(l);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let input = StageInput::new(
            snapshot,
            page_id.unwrap(),
            None,
            None,
            Arc::new(crate::ImageCache::default()),
            None,
        );
        let targets = collect_ocr_targets(&input).await.unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].content, content_id.unwrap());
        assert_eq!(targets[0].region, layer_id.unwrap());
    }
}
