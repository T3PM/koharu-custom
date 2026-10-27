use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result, bail};
use koharu_scene::{
    CharacterProfile, EntityId, GLOSSARY_EXAMPLE_MAX_CHARS, GLOSSARY_MAX_EXAMPLES,
    GLOSSARY_TEXT_MAX_CHARS, Glossary, GlossaryEntry, GlossaryEntryId, GlossaryKind,
    GlossaryValueOrigin, LanguageTag, Origin, Revision, Snapshot, SourceText, Speaker,
    normalize_glossary_source,
};

use super::scan::GlossaryScanInput;

pub(crate) const JP_HONORIFICS: &[&str] = &[
    "さん", "ちゃん", "くん", "君", "さま", "様", "先生", "せんせい", "先輩", "せんぱい", "氏", "殿", "たん",
];
pub(crate) const LATIN_HONORIFICS: &[&str] = &[
    "-san", "-chan", "-kun", "-sama", "-senpai", "-sensei", "-dono", "-tan", "-shi",
];
pub(crate) const KATA_HONORIFICS: &[&str] = &["サン", "チャン", "クン", "サマ", "センパイ", "センセイ"];
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Deserialize, Serialize, Type)]
pub(crate) struct OcrSourceRecord {
    #[specta(type = f64)]
    pub page_order: usize,
    pub content: EntityId,
    pub source_language: Option<String>,
    pub source_text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlossaryView {
    pub revision: Revision,
    pub enabled: bool,
    pub stale: bool,
    pub source_language: Option<LanguageTag>,
    pub target_language: Option<LanguageTag>,
    pub saved_source_fingerprint: Option<String>,
    pub current_source_fingerprint: String,
    pub entries: Vec<GlossaryEntryView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlossaryScanReport {
    pub glossary: GlossaryView,
    pub summary: GlossaryScanSummary,
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(
    tag = "status",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub(crate) enum GlossaryScanSummary {
    NoSourceText,
    NoTermsFound {
        source_text_count: u32,
    },
    Updated {
        source_text_count: u32,
        candidate_occurrence_count: u32,
        distinct_term_count: u32,
        added_entry_count: u32,
        matched_entry_count: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlossaryEntryView {
    pub revision: Revision,
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
    pub character: Option<CharacterProfile>,
    #[serde(default)]
    pub forbidden_variants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GlossaryEntryDraft {
    pub source: String,
    pub translation: Option<String>,
    pub kind: GlossaryKind,
    pub enabled: bool,
    pub character: Option<CharacterProfile>,
    #[serde(default)]
    pub forbidden_variants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GlossaryEntryPatch {
    pub source: String,
    pub translation: Option<String>,
    pub kind: GlossaryKind,
    pub enabled: bool,
    pub character: Option<CharacterProfile>,
    #[serde(default)]
    pub forbidden_variants: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub(crate) struct ScanCandidate {
    pub source: String,
    pub kind: GlossaryKind,
    pub confidence: f32,
    pub example: Option<String>,
}

impl ScanCandidate {
    pub(crate) fn validate(&self) -> Result<()> {
        validate_scan_text(&self.source, GLOSSARY_TEXT_MAX_CHARS, false, "source")?;
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            bail!("glossary confidence is invalid");
        }
        if let Some(example) = &self.example {
            validate_scan_text(example, GLOSSARY_EXAMPLE_MAX_CHARS, true, "example")?;
        }
        Ok(())
    }
}

fn validate_scan_text(value: &str, max_chars: usize, allow_empty: bool, field: &str) -> Result<()> {
    let valid_length = value.chars().count() <= max_chars;
    let has_prohibited_control = value.chars().any(char::is_control);
    let empty = normalize_glossary_source(value).is_empty();
    if valid_length && !has_prohibited_control && (allow_empty || !empty) {
        Ok(())
    } else {
        bail!("glossary {field} is invalid")
    }
}

pub(super) fn normalize_glossary_translation(translation: Option<String>) -> Option<String> {
    translation.and_then(|translation| {
        let translation = translation.trim();
        (!translation.is_empty()).then(|| translation.to_owned())
    })
}

fn normalize_forbidden_variants(variants: Vec<String>) -> Vec<String> {
    variants
        .into_iter()
        .map(|variant| variant.trim().to_owned())
        .filter(|variant| !variant.is_empty())
        .collect()
}

fn normalize_glossary_translations(glossary: &mut Glossary) {
    for entry in &mut glossary.entries {
        entry.translation = normalize_glossary_translation(entry.translation.take());
        if entry.translation.is_none() {
            entry.translation_origin = None;
        }
        entry.forbidden_variants =
            normalize_forbidden_variants(std::mem::take(&mut entry.forbidden_variants));
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Type)]
pub(crate) struct TermTranslationResult {
    pub id: GlossaryEntryId,
    pub source: String,
    pub translation: String,
}

pub(crate) fn ocr_fingerprint(records: impl IntoIterator<Item = OcrSourceRecord>) -> String {
    let mut records = records.into_iter().collect::<Vec<_>>();
    records.sort_unstable();

    let mut hasher = blake3::Hasher::new();
    hasher.update(b"koharu-ocr-source-fingerprint-v1\0");
    for record in records {
        hasher.update(&(record.page_order as u64).to_le_bytes());
        hasher.update(record.content.as_uuid().as_bytes());
        hash_optional_string(&mut hasher, record.source_language.as_deref());
        hash_string(&mut hasher, &record.source_text);
    }
    hasher.finalize().to_hex().to_string()
}

pub(crate) fn ocr_source_fingerprint(snapshot: &Snapshot) -> Result<String> {
    let mut records = Vec::new();
    for (page_order, page) in snapshot.pages().enumerate() {
        for entity in snapshot.subtree(page.id())? {
            let Some(source) = snapshot.component::<SourceText>(entity.id())? else {
                continue;
            };
            records.push(OcrSourceRecord {
                page_order,
                content: entity.id(),
                source_language: source.language.map(|language| language.to_string()),
                source_text: source.text.value,
            });
        }
    }
    Ok(ocr_fingerprint(records))
}

pub(super) fn glossary(snapshot: &Snapshot) -> Result<Glossary> {
    Ok(snapshot
        .project_component::<Glossary>()?
        .unwrap_or(Glossary {
            enabled: false,
            source_language: None,
            target_language: None,
            source_fingerprint: None,
            entries: Vec::new(),
        }))
}

fn glossary_view(snapshot: &Snapshot) -> Result<GlossaryView> {
    let revision = snapshot.revision();
    let current_source_fingerprint = ocr_source_fingerprint(snapshot)?;
    let glossary = glossary(snapshot)?;
    let stale = glossary.source_fingerprint.as_deref() != Some(&current_source_fingerprint);
    Ok(GlossaryView {
        revision,
        enabled: glossary.enabled,
        stale,
        source_language: glossary.source_language,
        target_language: glossary.target_language,
        saved_source_fingerprint: glossary.source_fingerprint,
        current_source_fingerprint,
        entries: glossary
            .entries
            .into_iter()
            .map(|entry| GlossaryEntryView {
                revision,
                id: entry.id,
                source: entry.source,
                translation: entry.translation,
                kind: entry.kind,
                enabled: entry.enabled,
                confidence: entry.confidence,
                occurrence_count: entry.occurrence_count,
                examples: entry.examples,
                source_origin: entry.source_origin,
                translation_origin: entry.translation_origin,
                present_in_last_scan: entry.present_in_last_scan,
                character: entry.character,
                forbidden_variants: entry.forbidden_variants,
            })
            .collect(),
    })
}

impl crate::commands::project::Project {
    pub(crate) fn glossary_view(&self) -> Result<GlossaryView> {
        glossary_view(&self.snapshot())
    }

    pub(crate) async fn set_glossary_enabled(
        &mut self,
        expected_revision: Revision,
        enabled: bool,
    ) -> Result<GlossaryView> {
        self.mutate_glossary(expected_revision, |glossary| {
            glossary.enabled = enabled;
            Ok(())
        })
        .await
    }

    pub(crate) async fn add_glossary_entry(
        &mut self,
        expected_revision: Revision,
        input: GlossaryEntryDraft,
    ) -> Result<GlossaryView> {
        self.mutate_glossary(expected_revision, |glossary| {
            let id = GlossaryEntryId::new();
            let character = input.character.map(|mut c| {
                c.id = id;
                c
            });
            if let Some(c) = &character {
                c.validate()?;
            }
            glossary.entries.push(GlossaryEntry {
                id,
                source: input.source,
                translation_origin: input
                    .translation
                    .as_ref()
                    .map(|_| GlossaryValueOrigin::User),
                translation: input.translation,
                kind: input.kind,
                enabled: input.enabled,
                confidence: None,
                occurrence_count: 0,
                examples: Vec::new(),
                source_origin: GlossaryValueOrigin::User,
                present_in_last_scan: false,
                character,
                forbidden_variants: normalize_forbidden_variants(input.forbidden_variants),
            });
            Ok(())
        })
        .await
    }

    pub(crate) async fn update_glossary_entry(
        &mut self,
        expected_revision: Revision,
        id: GlossaryEntryId,
        input: GlossaryEntryPatch,
    ) -> Result<GlossaryView> {
        self.mutate_glossary(expected_revision, |glossary| {
            let entry = glossary
                .entries
                .iter_mut()
                .find(|entry| entry.id == id)
                .context("glossary entry was not found")?;
            entry.source = input.source;
            entry.translation_origin = input
                .translation
                .as_ref()
                .map(|_| GlossaryValueOrigin::User);
            entry.translation = input.translation;
            entry.kind = input.kind;
            entry.enabled = input.enabled;
            entry.source_origin = GlossaryValueOrigin::User;
            if let Some(mut c) = input.character {
                c.id = id;
                c.validate()?;
                entry.character = Some(c);
            }
            entry.forbidden_variants = normalize_forbidden_variants(input.forbidden_variants);
            Ok(())
        })
        .await
    }

    pub(crate) async fn update_character_profile(
        &mut self,
        expected_revision: Revision,
        entry_id: GlossaryEntryId,
        profile: Option<CharacterProfile>,
    ) -> Result<GlossaryView> {
        self.mutate_glossary(expected_revision, |glossary| {
            let entry = glossary
                .entries
                .iter_mut()
                .find(|entry| entry.id == entry_id)
                .context("glossary entry was not found")?;
            if let Some(mut p) = profile {
                p.id = entry_id;
                p.validate()?;
                entry.character = Some(p);
            } else {
                entry.character = None;
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn update_entry_kinds(
        &mut self,
        expected_revision: Revision,
        updates: Vec<(GlossaryEntryId, GlossaryKind)>,
    ) -> Result<GlossaryView> {
        let update_map = updates.into_iter().collect::<HashMap<_, _>>();
        self.mutate_glossary(expected_revision, |glossary| {
            for entry in &mut glossary.entries {
                if let Some(&new_kind) = update_map.get(&entry.id) {
                    entry.kind = new_kind;
                }
            }
            Ok(())
        })
        .await
    }

    pub(crate) async fn delete_glossary_entries(
        &mut self,
        expected_revision: Revision,
        ids: Vec<GlossaryEntryId>,
    ) -> Result<GlossaryView> {
        let ids = ids.into_iter().collect::<HashSet<_>>();
        if ids.is_empty() {
            bail!("at least one glossary entry must be selected for deletion");
        }
        let snapshot = self.snapshot();
        if snapshot.revision() != expected_revision {
            bail!(
                "glossary edit expected revision {expected_revision}, but the project is at {}",
                snapshot.revision()
            );
        }
        let mut value = glossary(&snapshot)?;
        let found = value
            .entries
            .iter()
            .filter(|entry| ids.contains(&entry.id))
            .count();
        if found != ids.len() {
            bail!("one or more glossary entries were not found");
        }
        value.entries.retain(|entry| !ids.contains(&entry.id));
        normalize_glossary_translations(&mut value);

        let patch = snapshot.patch(|edit| {
            edit.set_project(&value)?;
            for entity in snapshot.entities_with::<Speaker>()? {
                if let Some(speaker) = snapshot.component::<Speaker>(entity.id())? {
                    if let Some(entry_id) = speaker.speaker {
                        if ids.contains(&entry_id) {
                            edit.set_speaker(entity.id(), None, Origin::User)?;
                        }
                    }
                }
            }
            Ok(())
        })?;
        let commit = self.session.commit(patch).await?;
        self.record_commit(&commit);
        glossary_view(&commit.snapshot)
    }

    pub(crate) async fn apply_glossary_scan(
        &mut self,
        expected_revision: Revision,
        input: GlossaryScanInput,
    ) -> Result<GlossaryScanReport> {
        #[derive(Debug)]
        struct Aggregate {
            source: String,
            kind: GlossaryKind,
            confidence: f32,
            occurrence_count: u32,
            examples: Vec<String>,
        }

        let snapshot = self.snapshot();
        if snapshot.revision() != expected_revision {
            bail!(
                "glossary scan expected revision {expected_revision}, but the project is at {}",
                snapshot.revision()
            );
        }
        if input.source_text_count == 0 {
            if !input.candidates.is_empty() {
                bail!("glossary scan candidates require OCR text");
            }
            return Ok(GlossaryScanReport {
                glossary: self.glossary_view()?,
                summary: GlossaryScanSummary::NoSourceText,
            });
        }

        let source_fingerprint = ocr_source_fingerprint(&snapshot)?;
        let candidate_occurrence_count =
            u32::try_from(input.candidates.len()).context("too many glossary scan matches")?;
        let mut positions = HashMap::<(String, GlossaryKind), usize>::new();
        let mut aggregates = Vec::<Aggregate>::new();
        for candidate in input.candidates {
            candidate.validate()?;
            let key = (normalize_glossary_source(&candidate.source), candidate.kind);
            if let Some(&index) = positions.get(&key) {
                let aggregate = &mut aggregates[index];
                if candidate.confidence > aggregate.confidence {
                    aggregate.confidence = candidate.confidence;
                }
                aggregate.occurrence_count = aggregate.occurrence_count.saturating_add(1);
                if let Some(example) = candidate.example
                    && aggregate.examples.len() < GLOSSARY_MAX_EXAMPLES
                    && !aggregate.examples.contains(&example)
                {
                    aggregate.examples.push(example);
                }
            } else {
                let mut examples = Vec::new();
                if let Some(example) = candidate.example {
                    examples.push(example);
                }
                positions.insert(key, aggregates.len());
                aggregates.push(Aggregate {
                    source: candidate.source,
                    kind: candidate.kind,
                    confidence: candidate.confidence,
                    occurrence_count: 1,
                    examples,
                });
            }
        }

        let distinct_term_count =
            u32::try_from(aggregates.len()).context("too many distinct glossary scan terms")?;
        let mut added_entry_count = 0_u32;
        let mut matched_entry_count = 0_u32;
        let glossary = self
            .mutate_glossary(expected_revision, |glossary| {
                if let Some(source) = input.source_language {
                    glossary.source_language = Some(source);
                }
                glossary.target_language = Some(input.target_language);
                glossary.source_fingerprint = Some(source_fingerprint);
                for entry in &mut glossary.entries {
                    entry.present_in_last_scan = false;
                }

                let existing = glossary
                    .entries
                    .iter()
                    .enumerate()
                    .map(|(index, entry)| {
                        (
                            (normalize_glossary_source(&entry.source), entry.kind),
                            index,
                        )
                    })
                    .collect::<HashMap<_, _>>();
                for aggregate in aggregates {
                    let key = (normalize_glossary_source(&aggregate.source), aggregate.kind);
                    if let Some(&index) = existing.get(&key) {
                        matched_entry_count = matched_entry_count.saturating_add(1);
                        let entry = &mut glossary.entries[index];
                        if !matches!(
                            entry.source_origin,
                            GlossaryValueOrigin::User | GlossaryValueOrigin::Imported
                        ) {
                            entry.source = aggregate.source;
                        }
                        entry.confidence = Some(aggregate.confidence);
                        entry.occurrence_count = aggregate.occurrence_count;
                        entry.examples = aggregate.examples;
                        entry.present_in_last_scan = true;
                    } else {
                        added_entry_count = added_entry_count.saturating_add(1);
                        glossary.entries.push(GlossaryEntry {
                            id: GlossaryEntryId::new(),
                            source: aggregate.source,
                            translation: None,
                            kind: aggregate.kind,
                            enabled: true,
                            confidence: Some(aggregate.confidence),
                            occurrence_count: aggregate.occurrence_count,
                            examples: aggregate.examples,
                            source_origin: GlossaryValueOrigin::Detected,
                            translation_origin: None,
                            present_in_last_scan: true,
                            character: None,
                            forbidden_variants: Vec::new(),
                        });
                    }
                }
                Ok(())
            })
            .await?;

        let summary = if distinct_term_count == 0 {
            GlossaryScanSummary::NoTermsFound {
                source_text_count: input.source_text_count,
            }
        } else {
            GlossaryScanSummary::Updated {
                source_text_count: input.source_text_count,
                candidate_occurrence_count,
                distinct_term_count,
                added_entry_count,
                matched_entry_count,
            }
        };
        Ok(GlossaryScanReport { glossary, summary })
    }

    pub(crate) async fn apply_term_translations(
        &mut self,
        expected_revision: Revision,
        translations: Vec<TermTranslationResult>,
    ) -> Result<GlossaryView> {
        let translations = translations
            .into_iter()
            .map(|translation| (translation.id, translation))
            .collect::<HashMap<_, _>>();
        self.mutate_glossary(expected_revision, |glossary| {
            for entry in &mut glossary.entries {
                let Some(result) = translations.get(&entry.id) else {
                    continue;
                };
                if entry.source != result.source
                    || !matches!(
                        entry.translation_origin,
                        None | Some(GlossaryValueOrigin::Automatic)
                    )
                {
                    continue;
                }
                entry.translation = Some(result.translation.clone());
                entry.translation_origin = Some(GlossaryValueOrigin::Automatic);
            }
            Ok(())
        })
        .await
    }

    pub(super) async fn mutate_glossary(
        &mut self,
        expected_revision: Revision,
        mutate: impl FnOnce(&mut Glossary) -> Result<()>,
    ) -> Result<GlossaryView> {
        let snapshot = self.snapshot();
        if snapshot.revision() != expected_revision {
            bail!(
                "glossary edit expected revision {expected_revision}, but the project is at {}",
                snapshot.revision()
            );
        }
        let mut value = glossary(&snapshot)?;
        mutate(&mut value)?;
        normalize_glossary_translations(&mut value);
        let patch = snapshot.patch(|edit| edit.set_project(&value))?;
        let commit = self.session.commit(patch).await?;
        self.record_commit(&commit);
        glossary_view(&commit.snapshot)
    }
}

fn hash_optional_string(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_string(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn hash_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use koharu_scene::{
        At, Authored, EntityId, GLOSSARY_EXAMPLE_MAX_CHARS, GLOSSARY_TEXT_MAX_CHARS, GlossaryEntryId,
        LanguageTag, Origin, PageDraft, RemovePolicy, Session, SourceText, Speaker, TextLayout,
        TextLayoutKind, Translation, Visibility,
    };

    use super::{
        GlossaryScanInput, GlossaryScanSummary, OcrSourceRecord, ocr_fingerprint,
        ocr_source_fingerprint,
    };
    use crate::commands::{
        glossary::{
            GlossaryEntryDraft, GlossaryEntryPatch, GlossaryView, ScanCandidate,
            TermTranslationResult,
        },
        project::Project,
    };
    use koharu_scene::{GlossaryKind, GlossaryValueOrigin, Revision};

    fn record(
        page_order: usize,
        content: EntityId,
        language: Option<&str>,
        text: &str,
    ) -> OcrSourceRecord {
        OcrSourceRecord {
            page_order,
            content,
            source_language: language.map(str::to_owned),
            source_text: text.to_owned(),
        }
    }

    #[test]
    fn fingerprint_is_deterministic_and_order_independent() {
        let first = EntityId::new();
        let second = EntityId::new();
        let records = vec![
            record(1, second, Some("ja"), "二"),
            record(0, first, Some("ja"), "一"),
        ];
        let reversed = records.iter().cloned().rev().collect::<Vec<_>>();

        let fingerprint = ocr_fingerprint(records);
        assert_eq!(fingerprint, ocr_fingerprint(reversed));
        assert_eq!(fingerprint.len(), 64);
        assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(fingerprint, fingerprint.to_ascii_lowercase());
    }

    #[tokio::test]
    async fn scene_fingerprint_tracks_only_ordered_ocr_sources() {
        let mut session = Session::memory().await.unwrap();
        let empty = ocr_source_fingerprint(&session.snapshot()).unwrap();
        assert_eq!(empty, ocr_source_fingerprint(&session.snapshot()).unwrap());

        let mut ids = None;
        let setup = session
            .snapshot()
            .patch(|edit| {
                let first_page = edit.add_page(PageDraft::new("one", 100.0, 100.0), At::End)?;
                let first = edit.add_text_content(first_page, At::End)?;
                edit.set(
                    first,
                    &SourceText {
                        text: Authored::user("一".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                let second_page = edit.add_page(PageDraft::new("two", 100.0, 100.0), At::End)?;
                let second = edit.add_text_content(second_page, At::End)?;
                edit.set(
                    second,
                    &SourceText {
                        text: Authored::user("二".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                ids = Some((first_page, first, second_page, second));
                Ok(())
            })
            .unwrap();
        session.commit(setup).await.unwrap();
        let (first_page, first, second_page, _) = ids.unwrap();
        let baseline = ocr_source_fingerprint(&session.snapshot()).unwrap();
        assert_ne!(baseline, empty);

        let presentation_only = session
            .snapshot()
            .patch(|edit| {
                edit.set(
                    first,
                    &Translation {
                        text: Authored::user("one".to_owned()),
                        language: Some(LanguageTag::new("en")?),
                    },
                )?;
                let layer = edit.add_text_layer(
                    first_page,
                    At::End,
                    first,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set(
                    layer,
                    &Visibility {
                        origin: Origin::User,
                        visible: false,
                        opacity: 0.5,
                    },
                )
            })
            .unwrap();
        session.commit(presentation_only).await.unwrap();
        assert_eq!(
            ocr_source_fingerprint(&session.snapshot()).unwrap(),
            baseline
        );

        let change_text = session
            .snapshot()
            .patch(|edit| {
                edit.set(
                    first,
                    &SourceText {
                        text: Authored::user(" changed ".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )
            })
            .unwrap();
        session.commit(change_text).await.unwrap();
        assert_ne!(
            ocr_source_fingerprint(&session.snapshot()).unwrap(),
            baseline
        );

        let change_language = session
            .snapshot()
            .patch(|edit| {
                edit.set(
                    first,
                    &SourceText {
                        text: Authored::user("一".to_owned()),
                        language: Some(LanguageTag::new("zh")?),
                    },
                )
            })
            .unwrap();
        session.commit(change_language).await.unwrap();
        assert_ne!(
            ocr_source_fingerprint(&session.snapshot()).unwrap(),
            baseline
        );

        let mut added = None;
        let add_source = session
            .snapshot()
            .patch(|edit| {
                edit.set(
                    first,
                    &SourceText {
                        text: Authored::user("一".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                let content = edit.add_text_content(first_page, At::End)?;
                edit.set(
                    content,
                    &SourceText {
                        text: Authored::user("三".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                added = Some(content);
                Ok(())
            })
            .unwrap();
        session.commit(add_source).await.unwrap();
        assert_ne!(
            ocr_source_fingerprint(&session.snapshot()).unwrap(),
            baseline
        );

        let remove_source = session
            .snapshot()
            .patch(|edit| edit.remove_entity(added.unwrap(), RemovePolicy::Cascade))
            .unwrap();
        session.commit(remove_source).await.unwrap();
        assert_eq!(
            ocr_source_fingerprint(&session.snapshot()).unwrap(),
            baseline
        );

        let reorder = session
            .snapshot()
            .patch(|edit| edit.move_entity(second_page, None, At::Start))
            .unwrap();
        session.commit(reorder).await.unwrap();
        assert_ne!(
            ocr_source_fingerprint(&session.snapshot()).unwrap(),
            baseline
        );
    }

    fn entry(source: &str, translation: Option<&str>) -> GlossaryEntryDraft {
        GlossaryEntryDraft {
            source: source.to_owned(),
            translation: translation.map(str::to_owned),
            kind: GlossaryKind::Person,
            enabled: true,
            character: None,
            forbidden_variants: Vec::new(),
        }
    }

    fn assert_view_revision(view: &GlossaryView, revision: Revision) {
        assert_eq!(view.revision, revision);
        assert!(view.entries.iter().all(|entry| entry.revision == revision));
    }

    #[tokio::test]
    async fn project_glossary_mutations_are_revision_guarded_and_record_one_commit() {
        let mut project = Project::new_test(Session::memory().await.unwrap(), "test".to_owned());
        let initial = project.glossary_view().unwrap();
        assert_view_revision(&initial, Revision::ZERO);
        assert!(!initial.enabled);
        assert!(initial.stale);
        assert_eq!(initial.source_language, None);
        assert_eq!(initial.target_language, None);
        assert_eq!(initial.saved_source_fingerprint, None);
        assert_eq!(initial.entries, Vec::new());

        let added = project
            .add_glossary_entry(Revision::ZERO, entry("アリス", Some("Alice")))
            .await
            .unwrap();
        assert_view_revision(&added, Revision::new(1));
        assert_eq!(project.undo.len(), 1);
        assert_eq!(added.entries.len(), 1);
        assert_eq!(added.entries[0].source_origin, GlossaryValueOrigin::User);
        assert_eq!(
            added.entries[0].translation_origin,
            Some(GlossaryValueOrigin::User)
        );
        let id = added.entries[0].id;

        let error = project
            .set_glossary_enabled(Revision::ZERO, true)
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "glossary edit expected revision 0, but the project is at 1"
        );
        assert_eq!(project.revision(), Revision::new(1));
        assert_eq!(project.undo.len(), 1);

        let enabled = project
            .set_glossary_enabled(Revision::new(1), true)
            .await
            .unwrap();
        assert_view_revision(&enabled, Revision::new(2));
        assert!(enabled.enabled);
        assert_eq!(project.undo.len(), 2);

        let updated = project
            .update_glossary_entry(
                Revision::new(2),
                id,
                GlossaryEntryPatch {
                    source: "アリシア".to_owned(),
                    translation: None,
                    kind: GlossaryKind::Term,
                    enabled: false,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert_view_revision(&updated, Revision::new(3));
        assert_eq!(updated.entries[0].source, "アリシア");
        assert_eq!(updated.entries[0].translation, None);
        assert_eq!(updated.entries[0].kind, GlossaryKind::Term);
        assert!(!updated.entries[0].enabled);
        assert_eq!(project.undo.len(), 3);

        let invalid_revision = project.revision();
        assert!(
            project
                .add_glossary_entry(invalid_revision, entry(" \t", None))
                .await
                .is_err()
        );
        assert_eq!(project.revision(), invalid_revision);
        assert_eq!(project.undo.len(), 3);

        let deleted = project
            .delete_glossary_entries(Revision::new(3), vec![id])
            .await
            .unwrap();
        assert_view_revision(&deleted, Revision::new(4));
        assert!(deleted.entries.is_empty());
        assert_eq!(project.undo.len(), 4);
    }

    #[tokio::test]
    async fn glossary_mutations_store_whitespace_only_translations_as_untranslated() {
        let mut project = Project::new_test(Session::memory().await.unwrap(), "test".to_owned());
        let added = project
            .add_glossary_entry(Revision::ZERO, entry("アリス", Some(" \u{2003} ")))
            .await
            .unwrap();
        let id = added.entries[0].id;
        assert_eq!(added.entries[0].translation, None);
        assert_eq!(added.entries[0].translation_origin, None);

        let translated = project
            .update_glossary_entry(
                added.revision,
                id,
                GlossaryEntryPatch {
                    source: "アリス".to_owned(),
                    translation: Some(" Alice ".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert_eq!(translated.entries[0].translation.as_deref(), Some("Alice"));

        let cleared = project
            .update_glossary_entry(
                translated.revision,
                id,
                GlossaryEntryPatch {
                    source: "アリス".to_owned(),
                    translation: Some(" \u{2003} ".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        assert_eq!(cleared.entries[0].translation, None);
        assert_eq!(cleared.entries[0].translation_origin, None);
    }

    fn stored_entry(
        source: &str,
        translation: Option<&str>,
        kind: GlossaryKind,
        enabled: bool,
        source_origin: GlossaryValueOrigin,
    ) -> koharu_scene::GlossaryEntry {
        koharu_scene::GlossaryEntry {
            id: koharu_scene::GlossaryEntryId::new(),
            source: source.to_owned(),
            translation: translation.map(str::to_owned),
            kind,
            enabled,
            confidence: Some(0.25),
            occurrence_count: 7,
            examples: vec!["old example".to_owned()],
            source_origin,
            translation_origin: translation.map(|_| source_origin),
            present_in_last_scan: true,
            character: None,
            forbidden_variants: Vec::new(),
        }
    }

    fn scan_candidate(source: &str, confidence: f32, example: Option<String>) -> ScanCandidate {
        ScanCandidate {
            source: source.to_owned(),
            kind: GlossaryKind::Person,
            confidence,
            example,
        }
    }

    fn scan_input(candidates: Vec<ScanCandidate>) -> GlossaryScanInput {
        GlossaryScanInput {
            source_text_count: 1,
            source_language: Some(LanguageTag::new("ja").unwrap()),
            target_language: LanguageTag::new("en").unwrap(),
            candidates,
        }
    }

    async fn assert_scan_rejected(candidates: Vec<ScanCandidate>, expected_error: &str) {
        let mut project = Project::new_test(Session::memory().await.unwrap(), "test".to_owned());
        let revision = project.revision();

        let error = project
            .apply_glossary_scan(revision, scan_input(candidates))
            .await
            .unwrap_err();

        assert!(
            error.to_string().ends_with(expected_error),
            "expected {expected_error:?}, got {error:#}"
        );
        assert_eq!(project.revision(), revision);
        assert!(project.undo.is_empty());
    }

    #[tokio::test]
    async fn scan_without_ocr_text_reports_without_mutating_project() {
        let mut project = Project::new_test(Session::memory().await.unwrap(), "test".to_owned());
        let revision = project.revision();
        let report = project
            .apply_glossary_scan(
                revision,
                GlossaryScanInput {
                    source_text_count: 0,
                    source_language: None,
                    target_language: LanguageTag::new("en").unwrap(),
                    candidates: Vec::new(),
                },
            )
            .await
            .unwrap();

        assert_eq!(report.summary, GlossaryScanSummary::NoSourceText);
        assert_eq!(report.glossary.revision, revision);
        assert_eq!(project.revision(), revision);
        assert!(project.undo.is_empty());
    }

    #[tokio::test]
    async fn scan_with_text_but_no_candidates_completes_and_records_fingerprint() {
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let text = edit.add_text_content(page, At::End)?;
                edit.set(
                    text,
                    &SourceText {
                        text: Authored::user("the".to_owned()),
                        language: Some(LanguageTag::new("en")?),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(setup).await.unwrap().snapshot;
        let input = crate::commands::glossary::scan::collect_project_scan_input(
            &snapshot,
            LanguageTag::new("ja").unwrap(),
        )
        .unwrap();
        assert_eq!(input.source_text_count, 1);
        assert!(input.candidates.is_empty());
        let mut project = Project::new_test(session, "test".to_owned());
        let revision = project.revision();

        let report = project.apply_glossary_scan(revision, input).await.unwrap();

        assert_eq!(
            report.summary,
            GlossaryScanSummary::NoTermsFound {
                source_text_count: 1
            }
        );
        assert_eq!(report.glossary.revision, Revision::new(revision.get() + 1));
        assert_eq!(
            report.glossary.saved_source_fingerprint.as_deref(),
            Some(report.glossary.current_source_fingerprint.as_str())
        );
    }

    #[tokio::test]
    async fn scan_rejects_each_invalid_confidence_in_both_duplicate_orders() {
        for invalid_confidence in [f32::NAN, -0.01, 1.01] {
            for invalid_first in [true, false] {
                let valid = scan_candidate("alice", 0.5, None);
                let invalid = scan_candidate("ＡLICE", invalid_confidence, None);
                let candidates = if invalid_first {
                    vec![invalid, valid]
                } else {
                    vec![valid, invalid]
                };

                assert_scan_rejected(candidates, "glossary confidence is invalid").await;
            }
        }
    }

    #[tokio::test]
    async fn scan_rejects_each_invalid_source_before_duplicate_aggregation() {
        assert_scan_rejected(
            vec![scan_candidate(" \t", 0.5, None)],
            "glossary source is invalid",
        )
        .await;

        let valid_composed = "é".repeat(GLOSSARY_TEXT_MAX_CHARS / 2 + 1);
        let invalid_decomposed = "e\u{301}".repeat(GLOSSARY_TEXT_MAX_CHARS / 2 + 1);
        for invalid_source in ["alice\n".to_owned(), invalid_decomposed] {
            for invalid_first in [true, false] {
                let valid = if invalid_source.contains('\n') {
                    scan_candidate("alice", 0.5, None)
                } else {
                    scan_candidate(&valid_composed, 0.5, None)
                };
                let invalid = scan_candidate(&invalid_source, 0.5, None);
                let candidates = if invalid_first {
                    vec![invalid, valid]
                } else {
                    vec![valid, invalid]
                };

                assert_scan_rejected(candidates, "glossary source is invalid").await;
            }
        }
    }

    #[tokio::test]
    async fn scan_rejects_invalid_examples_after_the_aggregate_limit_is_full() {
        for invalid_example in [
            "x".repeat(GLOSSARY_EXAMPLE_MAX_CHARS + 1),
            "invalid\nexample".to_owned(),
        ] {
            let candidates = vec![
                scan_candidate("alice", 0.5, Some("first".to_owned())),
                scan_candidate("alice", 0.5, Some("second".to_owned())),
                scan_candidate("alice", 0.5, Some("third".to_owned())),
                scan_candidate("alice", 0.5, Some(invalid_example)),
            ];

            assert_scan_rejected(candidates, "glossary example is invalid").await;
        }
    }

    #[tokio::test]
    async fn scan_merge_aggregates_and_preserves_existing_semantics() {
        let manual = stored_entry(
            " Ａlice ",
            Some("Alicia"),
            GlossaryKind::Person,
            false,
            GlossaryValueOrigin::User,
        );
        let imported = stored_entry(
            "Magic  Sword",
            Some("魔法剣"),
            GlossaryKind::Item,
            false,
            GlossaryValueOrigin::Imported,
        );
        let absent = stored_entry(
            "Old",
            None,
            GlossaryKind::Term,
            true,
            GlossaryValueOrigin::Detected,
        );
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                edit.set_project(&koharu_scene::Glossary {
                    enabled: true,
                    source_language: None,
                    target_language: None,
                    source_fingerprint: Some("old".to_owned()),
                    entries: vec![manual.clone(), imported.clone(), absent.clone()],
                })
            })
            .unwrap();
        session.commit(setup).await.unwrap();
        let mut project = Project::new_test(session, "test".to_owned());
        let expected = project.revision();

        let report = project
            .apply_glossary_scan(
                expected,
                scan_input(vec![
                    ScanCandidate {
                        source: "alice".to_owned(),
                        kind: GlossaryKind::Person,
                        confidence: 0.6,
                        example: Some("first".to_owned()),
                    },
                    ScanCandidate {
                        source: "ＡLICE".to_owned(),
                        kind: GlossaryKind::Person,
                        confidence: 0.9,
                        example: Some("second".to_owned()),
                    },
                    ScanCandidate {
                        source: "alice".to_owned(),
                        kind: GlossaryKind::Person,
                        confidence: 0.7,
                        example: Some("first".to_owned()),
                    },
                    ScanCandidate {
                        source: "ＭＡＧＩＣ sword".to_owned(),
                        kind: GlossaryKind::Item,
                        confidence: 0.8,
                        example: None,
                    },
                    ScanCandidate {
                        source: "ボブ".to_owned(),
                        kind: GlossaryKind::Person,
                        confidence: 0.75,
                        example: Some("ボブさん".to_owned()),
                    },
                ]),
            )
            .await
            .unwrap();
        assert_eq!(
            report.summary,
            GlossaryScanSummary::Updated {
                source_text_count: 1,
                candidate_occurrence_count: 5,
                distinct_term_count: 3,
                added_entry_count: 1,
                matched_entry_count: 2,
            }
        );
        let view = report.glossary;

        assert_view_revision(&view, Revision::new(expected.get() + 1));
        assert!(view.enabled);
        assert!(!view.stale);
        assert_eq!(
            view.saved_source_fingerprint,
            Some(view.current_source_fingerprint.clone())
        );
        assert_eq!(view.source_language.unwrap().as_str(), "ja");
        assert_eq!(view.target_language.unwrap().as_str(), "en");
        assert_eq!(project.undo.len(), 1);
        assert_eq!(view.entries.len(), 4);

        let manual_view = view
            .entries
            .iter()
            .find(|entry| entry.id == manual.id)
            .unwrap();
        assert_eq!(manual_view.source, manual.source);
        assert_eq!(manual_view.kind, manual.kind);
        assert_eq!(manual_view.enabled, manual.enabled);
        assert_eq!(manual_view.translation, manual.translation);
        assert_eq!(manual_view.source_origin, GlossaryValueOrigin::User);
        assert_eq!(manual_view.confidence, Some(0.9));
        assert_eq!(manual_view.occurrence_count, 3);
        assert_eq!(manual_view.examples, ["first", "second"]);
        assert!(manual_view.present_in_last_scan);

        let imported_view = view
            .entries
            .iter()
            .find(|entry| entry.id == imported.id)
            .unwrap();
        assert_eq!(imported_view.source, imported.source);
        assert_eq!(imported_view.enabled, imported.enabled);
        assert_eq!(imported_view.translation, imported.translation);
        assert_eq!(imported_view.source_origin, GlossaryValueOrigin::Imported);
        assert_eq!(imported_view.occurrence_count, 1);
        assert!(imported_view.present_in_last_scan);

        let absent_view = view
            .entries
            .iter()
            .find(|entry| entry.id == absent.id)
            .unwrap();
        assert_eq!(absent_view.confidence, absent.confidence);
        assert_eq!(absent_view.occurrence_count, absent.occurrence_count);
        assert_eq!(absent_view.examples, absent.examples);
        assert!(!absent_view.present_in_last_scan);

        let added = view
            .entries
            .iter()
            .find(|entry| entry.source == "ボブ")
            .unwrap();
        assert!(added.enabled);
        assert_eq!(added.translation, None);
        assert_eq!(added.translation_origin, None);
        assert_eq!(added.source_origin, GlossaryValueOrigin::Detected);
        assert_eq!(added.confidence, Some(0.75));
        assert_eq!(added.occurrence_count, 1);
        assert_eq!(added.examples, ["ボブさん"]);
        assert!(added.present_in_last_scan);
    }

    #[tokio::test]
    async fn translation_results_preserve_concurrent_entry_edits() {
        let mut automatic = stored_entry(
            "アリス",
            Some("old automatic"),
            GlossaryKind::Person,
            true,
            GlossaryValueOrigin::Detected,
        );
        automatic.translation_origin = Some(GlossaryValueOrigin::Automatic);
        let mut user = stored_entry(
            "ボブ",
            Some("custom Bob"),
            GlossaryKind::Person,
            true,
            GlossaryValueOrigin::User,
        );
        user.translation_origin = Some(GlossaryValueOrigin::User);
        let changed = stored_entry(
            "変更後",
            None,
            GlossaryKind::Term,
            true,
            GlossaryValueOrigin::User,
        );
        let mut imported = stored_entry(
            "剣",
            Some("Blade"),
            GlossaryKind::Item,
            true,
            GlossaryValueOrigin::Imported,
        );
        imported.translation_origin = Some(GlossaryValueOrigin::Imported);

        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                edit.set_project(&koharu_scene::Glossary {
                    enabled: true,
                    source_language: Some(LanguageTag::new("ja")?),
                    target_language: Some(LanguageTag::new("en")?),
                    source_fingerprint: None,
                    entries: vec![
                        automatic.clone(),
                        user.clone(),
                        changed.clone(),
                        imported.clone(),
                    ],
                })
            })
            .unwrap();
        session.commit(setup).await.unwrap();
        let mut project = Project::new_test(session, "test".to_owned());
        let expected = project.revision();

        let view = project
            .apply_term_translations(
                expected,
                vec![
                    TermTranslationResult {
                        id: automatic.id,
                        source: automatic.source.clone(),
                        translation: "Alice".to_owned(),
                    },
                    TermTranslationResult {
                        id: user.id,
                        source: user.source.clone(),
                        translation: "Bob".to_owned(),
                    },
                    TermTranslationResult {
                        id: changed.id,
                        source: "変更前".to_owned(),
                        translation: "Changed".to_owned(),
                    },
                    TermTranslationResult {
                        id: imported.id,
                        source: imported.source.clone(),
                        translation: "Sword".to_owned(),
                    },
                    TermTranslationResult {
                        id: koharu_scene::GlossaryEntryId::new(),
                        source: "削除済み".to_owned(),
                        translation: "Deleted".to_owned(),
                    },
                ],
            )
            .await
            .unwrap();

        assert_eq!(view.revision, Revision::new(expected.get() + 1));
        assert_eq!(project.undo.len(), 1);
        let automatic_view = view
            .entries
            .iter()
            .find(|entry| entry.id == automatic.id)
            .unwrap();
        assert_eq!(automatic_view.translation.as_deref(), Some("Alice"));
        assert_eq!(
            automatic_view.translation_origin,
            Some(GlossaryValueOrigin::Automatic)
        );
        let user_view = view
            .entries
            .iter()
            .find(|entry| entry.id == user.id)
            .unwrap();
        assert_eq!(user_view.translation, user.translation);
        assert_eq!(user_view.translation_origin, user.translation_origin);
        let changed_view = view
            .entries
            .iter()
            .find(|entry| entry.id == changed.id)
            .unwrap();
        assert_eq!(changed_view.translation, None);
        let imported_view = view
            .entries
            .iter()
            .find(|entry| entry.id == imported.id)
            .unwrap();
        assert_eq!(imported_view.translation, imported.translation);
        assert_eq!(
            imported_view.translation_origin,
            imported.translation_origin
        );
    }

    #[tokio::test]
    async fn deleting_glossary_entry_clears_text_layer_and_content_speakers() {
        let mut project = Project::new_test(Session::memory().await.unwrap(), "test".to_owned());

        let added = project
            .add_glossary_entry(
                project.revision(),
                GlossaryEntryDraft {
                    source: "アリス".to_owned(),
                    translation: Some("Alice".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        let entry_id = added.entries[0].id;

        let mut layer_id = None;
        let mut content_id = None;
        let setup = project
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 800.0, 1200.0), At::End)?;
                let content = edit.add_text_content(page, At::End)?;
                edit.set(
                    content,
                    &SourceText {
                        text: Authored::user("不思議の国へようこそ".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                edit.set_speaker(content, Some(entry_id), Origin::User)?;

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

                layer_id = Some(layer);
                content_id = Some(content);
                Ok(())
            })
            .unwrap();
        project.session.commit(setup).await.unwrap();
        let layer = layer_id.unwrap();
        let content = content_id.unwrap();

        {
            let snap = project.snapshot();
            assert_eq!(
                snap.component::<Speaker>(layer).unwrap().and_then(|s| s.speaker),
                Some(entry_id)
            );
            assert_eq!(
                snap.component::<Speaker>(content).unwrap().and_then(|s| s.speaker),
                Some(entry_id)
            );
        }

        let deleted = project
            .delete_glossary_entries(project.revision(), vec![entry_id])
            .await
            .unwrap();
        assert!(deleted.entries.is_empty());

        {
            let snap = project.snapshot();
            let layer_speaker = snap.component::<Speaker>(layer).unwrap().and_then(|s| s.speaker);
            let content_speaker = snap.component::<Speaker>(content).unwrap().and_then(|s| s.speaker);

            assert_eq!(layer_speaker, None, "TextLayer speaker must be cleared to None");
            assert_eq!(content_speaker, None, "TextContent speaker must be cleared to None");

            let effective_speaker = layer_speaker.or(content_speaker);
            assert_eq!(effective_speaker, None, "Effective speaker fallback must yield None");
        }
    }

    #[tokio::test]
    async fn adversarial_stress_test_delete_glossary_entries_all_carrier_combinations_and_error_cases() {
        let mut project = Project::new_test(Session::memory().await.unwrap(), "stress-test".to_owned());

        // 1. Add 3 glossary entries: Alice, Bob, Charlie
        let g1 = project
            .add_glossary_entry(
                project.revision(),
                GlossaryEntryDraft {
                    source: "アリス".to_owned(),
                    translation: Some("Alice".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        let alice_id = g1.entries[0].id;

        let g2 = project
            .add_glossary_entry(
                project.revision(),
                GlossaryEntryDraft {
                    source: "ボブ".to_owned(),
                    translation: Some("Bob".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        let bob_id = g2.entries.iter().find(|e| e.source == "ボブ").unwrap().id;

        let g3 = project
            .add_glossary_entry(
                project.revision(),
                GlossaryEntryDraft {
                    source: "チャーリー".to_owned(),
                    translation: Some("Charlie".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            )
            .await
            .unwrap();
        let charlie_id = g3.entries.iter().find(|e| e.source == "チャーリー").unwrap().id;

        // 2. Setup carriers across Page 1 and Page 2:
        let mut content_only_alice = None;
        let mut content_only_bob = None;
        let mut layer_only_alice = None;
        let mut mixed_layer = None;
        let mut mixed_content = None;
        let mut layer_only_charlie = None;
        let mut page2_layer_alice = None;
        let mut page2_content_alice = None;
        let mut page2_layer_bob = None;
        let mut page2_content_bob = None;
        let mut none_content = None;

        let setup_patch = project
            .snapshot()
            .patch(|edit| {
                // Page 1
                let p1 = edit.add_page(PageDraft::new("p1", 800.0, 1200.0), At::End)?;

                // Carrier 1: TextContent only with Alice (no TextLayer)
                let c_alice = edit.add_text_content(p1, At::End)?;
                edit.set_speaker(c_alice, Some(alice_id), Origin::User)?;
                content_only_alice = Some(c_alice);

                // Carrier 2: TextContent only with Bob (no TextLayer)
                let c_bob = edit.add_text_content(p1, At::End)?;
                edit.set_speaker(c_bob, Some(bob_id), Origin::User)?;
                content_only_bob = Some(c_bob);

                // Carrier 3: TextLayer only with Alice (its content has NO speaker)
                let c_plain = edit.add_text_content(p1, At::End)?;
                let l_alice = edit.add_text_layer(
                    p1,
                    At::End,
                    c_plain,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(l_alice, Some(alice_id), Origin::User)?;
                layer_only_alice = Some(l_alice);

                // Carrier 4: Mixed: TextLayer has Bob, but TextContent has Alice!
                let c_mixed = edit.add_text_content(p1, At::End)?;
                edit.set_speaker(c_mixed, Some(alice_id), Origin::User)?;
                let l_mixed = edit.add_text_layer(
                    p1,
                    At::End,
                    c_mixed,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(l_mixed, Some(bob_id), Origin::User)?;
                mixed_layer = Some(l_mixed);
                mixed_content = Some(c_mixed);

                // Carrier 5: TextLayer only with Charlie
                let c_plain2 = edit.add_text_content(p1, At::End)?;
                let l_charlie = edit.add_text_layer(
                    p1,
                    At::End,
                    c_plain2,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(l_charlie, Some(charlie_id), Origin::User)?;
                layer_only_charlie = Some(l_charlie);

                // Carrier 6: Content with Speaker::new(None)
                let c_none = edit.add_text_content(p1, At::End)?;
                edit.set_speaker(c_none, None, Origin::User)?;
                none_content = Some(c_none);

                // Page 2
                let p2 = edit.add_page(PageDraft::new("p2", 800.0, 1200.0), At::End)?;

                // Carrier 7: Page 2 both Alice
                let p2_c_alice = edit.add_text_content(p2, At::End)?;
                edit.set_speaker(p2_c_alice, Some(alice_id), Origin::User)?;
                let p2_l_alice = edit.add_text_layer(
                    p2,
                    At::End,
                    p2_c_alice,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(p2_l_alice, Some(alice_id), Origin::User)?;
                page2_layer_alice = Some(p2_l_alice);
                page2_content_alice = Some(p2_c_alice);

                // Carrier 8: Page 2 both Bob
                let p2_c_bob = edit.add_text_content(p2, At::End)?;
                edit.set_speaker(p2_c_bob, Some(bob_id), Origin::User)?;
                let p2_l_bob = edit.add_text_layer(
                    p2,
                    At::End,
                    p2_c_bob,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(p2_l_bob, Some(bob_id), Origin::User)?;
                page2_layer_bob = Some(p2_l_bob);
                page2_content_bob = Some(p2_c_bob);

                Ok(())
            })
            .unwrap();
        project.session.commit(setup_patch).await.unwrap();

        let c_alice = content_only_alice.unwrap();
        let c_bob = content_only_bob.unwrap();
        let l_alice = layer_only_alice.unwrap();
        let l_mixed = mixed_layer.unwrap();
        let c_mixed = mixed_content.unwrap();
        let l_charlie = layer_only_charlie.unwrap();
        let p2_l_alice = page2_layer_alice.unwrap();
        let p2_c_alice = page2_content_alice.unwrap();
        let p2_l_bob = page2_layer_bob.unwrap();
        let p2_c_bob = page2_content_bob.unwrap();
        let c_none = none_content.unwrap();

        // Verify initial state
        {
            let snap = project.snapshot();
            assert_eq!(snap.component::<Speaker>(c_alice).unwrap().and_then(|s| s.speaker), Some(alice_id));
            assert_eq!(snap.component::<Speaker>(c_bob).unwrap().and_then(|s| s.speaker), Some(bob_id));
            assert_eq!(snap.component::<Speaker>(l_alice).unwrap().and_then(|s| s.speaker), Some(alice_id));
            assert_eq!(snap.component::<Speaker>(l_mixed).unwrap().and_then(|s| s.speaker), Some(bob_id));
            assert_eq!(snap.component::<Speaker>(c_mixed).unwrap().and_then(|s| s.speaker), Some(alice_id));
            assert_eq!(snap.component::<Speaker>(l_charlie).unwrap().and_then(|s| s.speaker), Some(charlie_id));
            assert_eq!(snap.component::<Speaker>(p2_l_alice).unwrap().and_then(|s| s.speaker), Some(alice_id));
            assert_eq!(snap.component::<Speaker>(p2_c_alice).unwrap().and_then(|s| s.speaker), Some(alice_id));
            assert_eq!(snap.component::<Speaker>(p2_l_bob).unwrap().and_then(|s| s.speaker), Some(bob_id));
            assert_eq!(snap.component::<Speaker>(p2_c_bob).unwrap().and_then(|s| s.speaker), Some(bob_id));
            assert_eq!(snap.component::<Speaker>(c_none).unwrap().and_then(|s| s.speaker), None);
        }

        // 3. Step A: Delete ONLY Alice
        project
            .delete_glossary_entries(project.revision(), vec![alice_id])
            .await
            .unwrap();

        {
            let snap = project.snapshot();
            // Content-only Alice cleared to None
            assert_eq!(snap.component::<Speaker>(c_alice).unwrap().and_then(|s| s.speaker), None);
            // Content-only Bob preserved
            assert_eq!(snap.component::<Speaker>(c_bob).unwrap().and_then(|s| s.speaker), Some(bob_id));
            // Layer-only Alice cleared to None
            assert_eq!(snap.component::<Speaker>(l_alice).unwrap().and_then(|s| s.speaker), None);
            // Mixed carrier: Layer (Bob) preserved, Content (Alice) cleared to None!
            assert_eq!(snap.component::<Speaker>(l_mixed).unwrap().and_then(|s| s.speaker), Some(bob_id));
            assert_eq!(snap.component::<Speaker>(c_mixed).unwrap().and_then(|s| s.speaker), None);
            // Layer Charlie preserved
            assert_eq!(snap.component::<Speaker>(l_charlie).unwrap().and_then(|s| s.speaker), Some(charlie_id));
            // Page 2 Alice carriers both cleared
            assert_eq!(snap.component::<Speaker>(p2_l_alice).unwrap().and_then(|s| s.speaker), None);
            assert_eq!(snap.component::<Speaker>(p2_c_alice).unwrap().and_then(|s| s.speaker), None);
            // Page 2 Bob carriers both preserved
            assert_eq!(snap.component::<Speaker>(p2_l_bob).unwrap().and_then(|s| s.speaker), Some(bob_id));
            assert_eq!(snap.component::<Speaker>(p2_c_bob).unwrap().and_then(|s| s.speaker), Some(bob_id));

            // CRITICAL EMPIRICAL SCAN: Ensure NO entity across entire scene holds alice_id
            for entity in snap.entities_with::<Speaker>().unwrap() {
                let sp = snap.component::<Speaker>(entity.id()).unwrap().and_then(|s| s.speaker);
                assert_ne!(sp, Some(alice_id), "Entity {:?} has dangling alice_id reference!", entity.id());
            }
        }

        // 4. Step B: Batch Delete [Bob, Charlie]
        project
            .delete_glossary_entries(project.revision(), vec![bob_id, charlie_id])
            .await
            .unwrap();

        {
            let snap = project.snapshot();
            // CRITICAL EMPIRICAL SCAN: Verify ALL remaining speaker references are cleared to None
            for entity in snap.entities_with::<Speaker>().unwrap() {
                let sp = snap.component::<Speaker>(entity.id()).unwrap().and_then(|s| s.speaker);
                assert_eq!(sp, None, "Entity {:?} has dangling speaker reference {:?}!", entity.id(), sp);
            }
        }

        // 5. Negative Stress Cases:
        // Case 5a: Non-existent entry ID
        let missing_entry = GlossaryEntryId::new();
        let err_missing = project
            .delete_glossary_entries(project.revision(), vec![missing_entry])
            .await;
        assert!(err_missing.is_err(), "Deleting missing glossary entry must fail");

        // Case 5b: Empty IDs list
        let err_empty = project
            .delete_glossary_entries(project.revision(), vec![])
            .await;
        assert!(err_empty.is_err(), "Deleting empty glossary IDs must fail");

        // Case 5c: Stale revision
        let stale_rev = Revision::new(999);
        let err_stale = project
            .delete_glossary_entries(stale_rev, vec![alice_id])
            .await;
        assert!(err_stale.is_err(), "Deleting with stale revision must fail");
    }
}
