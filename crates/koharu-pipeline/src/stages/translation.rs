use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use koharu_scene::{
    Authored, CharacterGender, EntityId, Glossary, GlossaryEntry, GlossaryEntryId, GlossaryKind,
    LanguageTag, Origin, Snapshot, SourceText, Speaker, Translation, normalize_glossary_source,
};
use koharu_translator::{TranslationContext, TranslationRequest, Translator};

use crate::TranslationConfig;

use super::{StageInput, StageProcessor, finish, generation};

const PRODUCER: &str = "dev.koharu.pipeline.translation";
const MAX_CONTEXT_ITEMS: usize = 20;
const MAX_CONTEXT_CHARS: usize = 2000;
const MAX_GLOSSARY_CHARS: usize = 3200;
const MAX_PERSONA_CHARS: usize = 2400;

pub(super) struct Processor {
    config: TranslationConfig,
    translator: Translator,
}

impl Processor {
    pub(super) fn new(config: TranslationConfig, translator: Translator) -> Self {
        Self { config, translator }
    }
}

#[async_trait]
impl StageProcessor for Processor {
    fn model(&self) -> &'static str {
        Translator::model(&self.config.model)
    }

    fn unload(&self) -> bool {
        self.translator.unload()
    }

    async fn load(&self) -> Result<()> {
        self.translator.load_model(&self.config.model).await
    }

    async fn process(&self, input: StageInput) -> Result<koharu_scene::Patch> {
        let targets = collect_translation_targets(&input)?;

        let glossary = input.scene.project_component::<Glossary>()?.filter(|g| g.enabled);

        let mut speaker_names = HashMap::new();
        let mut person_entries = Vec::new();
        if let Some(glossary) = &glossary {
            for entry in &glossary.entries {
                if entry.enabled && entry.kind == GlossaryKind::Person {
                    let name = entry
                        .translation
                        .as_deref()
                        .filter(|t| !t.trim().is_empty())
                        .unwrap_or(entry.source.as_str())
                        .trim()
                        .to_owned();
                    speaker_names.insert(entry.id, name);
                    person_entries.push(entry);
                }
            }
        }

        let segments = targets.iter().map(|(_, _, source, speaker, _)| {
            if self.config.enable_speaker_tagging
                && let Some(speaker_id) = speaker
                && let Some(name) = speaker_names.get(speaker_id)
            {
                format!("[Speaker: {name}] {source}")
            } else {
                source.clone()
            }
        });

        let mut request = TranslationRequest::new(segments, self.config.target_language);

        let context_items = collect_sliding_context(&input.scene, input.page, self.config.context_window_pages);
        if !context_items.is_empty() {
            request = request.with_context(context_items);
        }

        let mut instructions = self.config.instructions.clone().unwrap_or_default();

        if let Some(glossary) = &glossary {
            let page_sources: Vec<&str> = targets.iter().map(|(_, _, s, _, _)| s.as_str()).collect();
            let glossary_section = format_glossary_section(glossary, &page_sources);
            if !glossary_section.is_empty() {
                if !instructions.is_empty() {
                    instructions.push_str("\n\n");
                }
                instructions.push_str(&glossary_section);
            }

            if self.config.enable_personas && !person_entries.is_empty() {
                let persona_section = format_persona_context(
                    &input.scene,
                    input.page,
                    self.config.context_window_pages,
                    &targets,
                    &person_entries,
                );
                if !persona_section.is_empty() {
                    if !instructions.is_empty() {
                        instructions.push_str("\n\n");
                    }
                    instructions.push_str(&persona_section);
                }
            }
        }

        if self.config.enable_speaker_tagging {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str(
                "When a line begins with '[Speaker: <Name>]', adopt that character's voice and tone. Output ONLY the translated dialogue without the '[Speaker: ...]' tag.",
            );
        }

        if self.config.auto_guess_speakers {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str(
                "If a segment does not specify '[Speaker: ...]' but you can infer who is speaking from the dialogue or personas, prepend '[Speaker: <Name>]' to the translation.",
            );
        }

        let mut sfx_terms = Vec::new();
        for (_, _, source, _, is_sfx) in &targets {
            if *is_sfx {
                if let Some(en) =
                    koharu_translator::lookup_onomatopoeia(source, self.config.target_language)
                {
                    sfx_terms.push(format!("- {}: {en}", source.trim()));
                }
            }
        }
        if !sfx_terms.is_empty() {
            if !instructions.is_empty() {
                instructions.push_str("\n\n");
            }
            instructions.push_str("Sound effects (SFX) / Onomatopoeia reference:\n");
            instructions.push_str(&sfx_terms.join("\n"));
        }

        if !instructions.is_empty() {
            request = request.with_instructions(instructions);
        }
        if Translator::supports_vision(&self.config.model, &self.config.generation)
            && let Some(image) = input.images.get(&input.scene, input.page, "source").await?
        {
            request = request.with_image(image);
        }
        let (provider, translated) = self
            .translator
            .translate(&self.config.model, self.config.generation, request)
            .await?;
        let language = LanguageTag::new(self.config.target_language.tag())?;
        let generated = generation(PRODUCER, provider)?;
        let mut edit = input.scene.edit_as(generated.clone());
        for (content_id, layer_id, _, _, _) in &targets {
            edit.observe::<SourceText>(*content_id)?;
            edit.observe::<Translation>(*content_id)?;
            edit.observe::<Speaker>(*layer_id)?;
        }
        for ((content_id, layer_id, source, existing_speaker, is_sfx), mut text) in
            targets.into_iter().zip(translated)
        {
            if input
                .scene
                .component::<Translation>(content_id)?
                .is_some_and(|value| matches!(value.text.origin, Origin::User))
            {
                continue;
            }

            let trimmed_leading = text.trim_start();
            if trimmed_leading.to_ascii_lowercase().starts_with("[speaker:") {
                if let Some(end) = trimmed_leading.find(']') {
                    let speaker_name = trimmed_leading[9..end].trim().to_lowercase();
                    let rest = trimmed_leading[end + 1..].trim();
                    let cleaned = rest
                        .strip_prefix(':')
                        .or_else(|| rest.strip_prefix('：'))
                        .unwrap_or(rest)
                        .trim();
                    text = cleaned.to_owned();

                    if self.config.auto_guess_speakers && existing_speaker.is_none() {
                        if let Some(glossary) = &glossary {
                            if let Some(matched) = glossary.entries.iter().find(|e| {
                                e.enabled
                                    && e.kind == GlossaryKind::Person
                                    && (e.source.trim().to_lowercase() == speaker_name
                                        || e.translation
                                            .as_ref()
                                            .is_some_and(|t| t.trim().to_lowercase() == speaker_name))
                            }) {
                                edit.set_speaker(layer_id, Some(matched.id), Origin::Generated(generated.clone()))?;
                            }
                        }
                    }
                }
            }

            let text = if source.trim() == "\u{2026}" {
                "\u{2026}".to_owned()
            } else if is_sfx && (text.trim().is_empty() || text.trim() == source.trim()) {
                if let Some(direct) =
                    koharu_translator::lookup_onomatopoeia(&source, self.config.target_language)
                {
                    direct.to_owned()
                } else {
                    text
                }
            } else {
                text
            };


            edit.set(
                content_id,
                &Translation {
                    text: Authored::generated(text, generated.clone()),
                    language: Some(language.clone()),
                },
            )?;
        }
        finish(edit)
    }
}

pub(super) fn collect_translation_targets(
    input: &StageInput,
) -> Result<Vec<(EntityId, EntityId, String, Option<koharu_scene::GlossaryEntryId>, bool)>> {
    let mut targets = Vec::new();
    let mut visited = std::collections::HashSet::new();

    let mut collect_layer = |layer_id: EntityId| -> Result<()> {
        if !visited.insert(layer_id) {
            return Ok(());
        }
        if !input.contains_entity(layer_id)? {
            return Ok(());
        }
        let layer = input.scene.text_layer(layer_id)?;
        let content = layer.content()?;
        let source_text = match content.source()? {
            Some(s) if !s.text.value.trim().is_empty() => Some(s.text.value),
            _ => None,
        };
        let Some(source) = source_text else {
            return Ok(());
        };
        let speaker = layer.speaker()?.or(content.speaker()?).and_then(|s| s.speaker);
        let is_sfx = content
            .role()?
            .is_some_and(|r| r.role == "dev.koharu.text.sfx");
        targets.push((content.id(), layer.id(), source, speaker, is_sfx));
        Ok(())
    };

    if let Some(group) = input.scene.page(input.page)?.text_group()? {
        for layer in group.text_layers()? {
            collect_layer(layer.id())?;
        }
    }

    for entity in input.scene.descendants(input.page)? {
        let id = entity.id();
        if entity.component::<koharu_scene::TextLayout>()?.is_some() {
            collect_layer(id)?;
        }
    }

    Ok(targets)
}

pub(super) fn collect_sliding_context(
    scene: &Snapshot,
    current_page: EntityId,
    context_window_pages: u32,
) -> Vec<TranslationContext> {
    if context_window_pages == 0 {
        return Vec::new();
    }
    let pages = scene.pages().collect::<Vec<_>>();
    let Some(current_index) = pages.iter().position(|p| p.id() == current_page) else {
        return Vec::new();
    };
    let start_index = current_index.saturating_sub(context_window_pages as usize);
    let mut context_items = Vec::new();
    let mut total_chars = 0;
    for page_ref in pages[start_index..current_index].iter().rev() {
        let mut page_items = Vec::new();
        if let Ok(Some(group)) = page_ref.text_group() {
            if let Ok(layers) = group.text_layers() {
                for layer in layers {
                    if let Ok(content) = layer.content() {
                        if let (Ok(Some(source)), Ok(Some(trans))) =
                            (content.source(), content.translation())
                        {
                            let s = source.text.value.trim();
                            let t = trans.text.value.trim();
                            if !s.is_empty() && !t.is_empty() {
                                page_items.push((s.to_owned(), t.to_owned()));
                            }
                        }
                    }
                }
            }
        }
        let mut page_fits = Vec::new();
        for (s, t) in page_items {
            let item_chars = s.chars().count() + t.chars().count();
            if total_chars + item_chars <= MAX_CONTEXT_CHARS
                && context_items.len() + page_fits.len() < MAX_CONTEXT_ITEMS
            {
                total_chars += item_chars;
                page_fits.push(TranslationContext {
                    source: s,
                    translation: t,
                });
            }
        }
        page_fits.reverse();
        context_items.extend(page_fits);
        if total_chars >= MAX_CONTEXT_CHARS || context_items.len() >= MAX_CONTEXT_ITEMS {
            break;
        }
    }
    context_items.reverse();
    context_items
}

pub(super) fn format_glossary_section(glossary: &Glossary, page_sources: &[&str]) -> String {
    let normalized_page = normalize_glossary_source(&page_sources.join(" "));
    let mut entries: Vec<&GlossaryEntry> = glossary
        .entries
        .iter()
        .filter(|e| {
            e.enabled
                && e.translation
                    .as_ref()
                    .is_some_and(|t| !t.trim().is_empty())
        })
        .collect();
    if entries.is_empty() {
        return String::new();
    }
    entries.sort_by_key(|entry| {
        let norm_src = normalize_glossary_source(&entry.source);
        !(!norm_src.is_empty() && normalized_page.contains(&norm_src))
    });
    let mut terms = Vec::new();
    let mut total_chars = 0;
    for entry in entries {
        let Some(trans) = &entry.translation else {
            continue;
        };
        let line = format!("- {}: {}", entry.source.trim(), trans.trim());
        let line_chars = line.chars().count() + usize::from(!terms.is_empty());
        if total_chars + line_chars <= MAX_GLOSSARY_CHARS {
            total_chars += line_chars;
            terms.push(line);
        }
    }
    if terms.is_empty() {
        return String::new();
    }
    format!("Glossary / Character names reference:\n{}", terms.join("\n"))
}

pub(super) fn format_persona_context(
    scene: &Snapshot,
    current_page: EntityId,
    context_window_pages: u32,
    targets: &[(EntityId, EntityId, String, Option<GlossaryEntryId>, bool)],
    person_entries: &[&GlossaryEntry],
) -> String {
    let mut current_page_speakers = std::collections::BTreeSet::new();
    let mut window_speakers = std::collections::BTreeSet::new();
    let mut current_page_texts = Vec::new();
    let mut window_texts = Vec::new();

    for (_, _, source, speaker, _) in targets {
        current_page_texts.push(source.as_str());
        if let Some(id) = speaker {
            current_page_speakers.insert(*id);
        }
    }
    let normalized_current = normalize_glossary_source(&current_page_texts.join(" "));

    let pages = scene.pages().collect::<Vec<_>>();
    if let Some(current_index) = pages.iter().position(|p| p.id() == current_page) {
        let start_index = current_index.saturating_sub(context_window_pages as usize);
        for page_ref in &pages[start_index..=current_index] {
            if let Ok(descendants) = scene.descendants(page_ref.id()) {
                for entity in descendants {
                    let id = entity.id();
                    if let Ok(layer) = scene.text_layer(id) {
                        if let Some(speaker_id) =
                            layer.speaker().ok().flatten().and_then(|s| s.speaker)
                        {
                            window_speakers.insert(speaker_id);
                        }
                        if let Ok(content) = layer.content() {
                            if let Some(speaker_id) =
                                content.speaker().ok().flatten().and_then(|s| s.speaker)
                            {
                                window_speakers.insert(speaker_id);
                            }
                            if let Ok(Some(src)) = content.source() {
                                window_texts.push(src.text.value);
                            }
                        }
                    }
                }
            }
        }
    }
    let normalized_window = normalize_glossary_source(&window_texts.join(" "));

    let mut sorted_personas = person_entries.to_vec();
    sorted_personas.sort_by_key(|entry| {
        let norm_src = normalize_glossary_source(&entry.source);
        let on_current = current_page_speakers.contains(&entry.id)
            || (!norm_src.is_empty() && normalized_current.contains(&norm_src));
        let in_window = window_speakers.contains(&entry.id)
            || (!norm_src.is_empty() && normalized_window.contains(&norm_src));
        if on_current {
            0_u8
        } else if in_window {
            1_u8
        } else {
            2_u8
        }
    });

    let mut personas = Vec::new();
    let mut total_persona_chars = 0;
    for entry in &sorted_personas {
        let name = entry
            .translation
            .as_deref()
            .filter(|t| !t.trim().is_empty())
            .unwrap_or(entry.source.as_str())
            .trim();
        let mut parts = vec![format!("- Character: {name} (Source: {})", entry.source.trim())];
        if let Some(profile) = &entry.character {
            if profile.gender != CharacterGender::Unspecified {
                parts.push(format!("Gender: {:?}", profile.gender));
            }
            if let Some(fp) = &profile.first_person {
                parts.push(format!("1st-person: {fp}"));
            }
            if let Some(sp) = &profile.second_person {
                parts.push(format!("2nd-person: {sp}"));
            }
            if let Some(tone) = &profile.tone {
                parts.push(format!("Tone: {tone}"));
            }
            if let Some(ci) = &profile.custom_instructions {
                parts.push(format!("Notes: {ci}"));
            }
        }
        let line = parts.join(" | ");
        let line_chars = line.chars().count();
        if total_persona_chars + line_chars <= MAX_PERSONA_CHARS {
            total_persona_chars += line_chars;
            personas.push(line);
        }
    }
    if personas.is_empty() {
        return String::new();
    }
    format!("Character Personas & Speech Styles:\n{}", personas.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use koharu_scene::{At, PageDraft, Session, TextLayout, TextLayoutKind};

    #[tokio::test]
    async fn translation_collects_standalone_text_layers_outside_text_group() {
        let mut session = Session::memory().await.unwrap();
        let mut page_id = None;
        let mut content_id = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let content = edit.add_text_content(page, At::End)?;
                edit.set(
                    content,
                    &SourceText {
                        text: Authored::user("Hello world".to_owned()),
                        language: None,
                    },
                )?;
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
                page_id = Some(page);
                content_id = Some(content);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let page = page_id.unwrap();
        let content = content_id.unwrap();

        let input = StageInput::new(
            snapshot,
            page,
            None,
            None,
            std::sync::Arc::new(crate::ImageCache::default()),
            None,
        );

        let targets = collect_translation_targets(&input).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, content);
        assert_eq!(targets[0].2, "Hello world");
    }

    #[tokio::test]
    async fn sliding_context_preserves_multi_page_and_intra_page_reading_order() {
        let mut session = Session::memory().await.unwrap();
        let mut page3_id = None;
        let patch = session
            .snapshot()
            .patch(|edit| {
                let p1 = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                for (src, tr) in [("p1_line1", "t1_line1"), ("p1_line2", "t1_line2")] {
                    let c = edit.add_text_content(p1, At::End)?;
                    edit.set(
                        c,
                        &SourceText {
                            text: Authored::user(src.to_owned()),
                            language: None,
                        },
                    )?;
                    edit.set(
                        c,
                        &Translation {
                            text: Authored::user(tr.to_owned()),
                            language: None,
                        },
                    )?;
                    edit.add_text_layer(
                        p1,
                        At::End,
                        c,
                        &TextLayout {
                            origin: Origin::User,
                            kind: TextLayoutKind::Paragraph,
                            angle_degrees: None,
                        },
                    )?;
                }

                let p2 = edit.add_page(PageDraft::new("p2", 100.0, 100.0), At::End)?;
                for (src, tr) in [("p2_line1", "t2_line1"), ("p2_line2", "t2_line2")] {
                    let c = edit.add_text_content(p2, At::End)?;
                    edit.set(
                        c,
                        &SourceText {
                            text: Authored::user(src.to_owned()),
                            language: None,
                        },
                    )?;
                    edit.set(
                        c,
                        &Translation {
                            text: Authored::user(tr.to_owned()),
                            language: None,
                        },
                    )?;
                    edit.add_text_layer(
                        p2,
                        At::End,
                        c,
                        &TextLayout {
                            origin: Origin::User,
                            kind: TextLayoutKind::Paragraph,
                            angle_degrees: None,
                        },
                    )?;
                }

                let p3 = edit.add_page(PageDraft::new("p3", 100.0, 100.0), At::End)?;
                page3_id = Some(p3);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let p3 = page3_id.unwrap();

        let context = collect_sliding_context(&snapshot, p3, 2);
        let sources: Vec<&str> = context.iter().map(|c| c.source.as_str()).collect();
        assert_eq!(sources, vec!["p1_line1", "p1_line2", "p2_line1", "p2_line2"]);
    }
}

