use std::collections::{BTreeMap, BTreeSet, HashSet};

use koharu_scene::{
    EntityId, Glossary, GlossaryEntryId, GlossaryKind, Snapshot, TextLayerRef, TextLayout,
};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryLintSeverity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryLintKind {
    MissingGlossaryTerm,
    ForbiddenVariantUsed,
    UntranslatedSourceLeak,
    SpeakerFirstPersonMismatch,
    SpeakerHonorificConflict,
    CrossChapterSpeakerDrift,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GlossaryLintIssue {
    pub kind: GlossaryLintKind,
    pub severity: GlossaryLintSeverity,
    pub page_id: EntityId,
    #[specta(type = f64)]
    pub page_index: usize,
    pub page_label: String,
    pub chapter: Option<String>,
    pub layer_id: EntityId,
    pub entry_id: Option<GlossaryEntryId>,
    pub speaker_id: Option<GlossaryEntryId>,
    pub expected: String,
    pub actual: String,
    pub message: String,
}

struct BubbleContext {
    page_id: EntityId,
    page_index: usize,
    page_label: String,
    chapter: Option<String>,
    layer_id: EntityId,
    source_text: String,
    translation_text: String,
    speaker_id: Option<GlossaryEntryId>,
}

const TARGET_FIRST_PERSON_CANDIDATES: &[&str] = &[
    "I", "me", "my", "we", "us", "our", "私", "僕", "俺", "あたし", "わたくし", "うち", "我", "吾輩",
];

const HONORIFIC_MARKERS: &[&str] = &[
    "-san", "-chan", "-kun", "-sama", "-senpai", "-sensei", "-dono", "-tan", "-shi", "さん",
    "ちゃん", "くん", "さま", "様", "先生", "先輩", "殿",
];

fn ordered_page_text_layers<'a>(scene: &'a Snapshot, page_id: EntityId) -> Vec<TextLayerRef<'a>> {
    let Ok(page) = scene.page(page_id) else {
        return Vec::new();
    };
    let mut layers = Vec::new();
    let mut seen = HashSet::new();
    if let Ok(Some(group)) = page.text_group()
        && let Ok(group_layers) = group.text_layers()
    {
        for layer in group_layers {
            if seen.insert(layer.id()) {
                layers.push(layer);
            }
        }
    }
    if let Ok(descendants) = scene.descendants(page_id) {
        for entity in descendants {
            if entity
                .component::<TextLayout>()
                .ok()
                .flatten()
                .is_some()
                && seen.insert(entity.id())
                && let Ok(layer) = scene.text_layer(entity.id())
            {
                layers.push(layer);
            }
        }
    }
    layers
}

fn contains_case_insensitive_whole_word(haystack: &str, needle: &str) -> bool {
    let trimmed = needle.trim();
    if trimmed.is_empty() {
        return false;
    }
    let is_ascii_word = trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '\'' || ch == '-');
    if !is_ascii_word {
        let lower_haystack = haystack.to_lowercase();
        let lower_needle = trimmed.to_lowercase();
        return lower_haystack.contains(&lower_needle);
    }

    let haystack_chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = trimmed.chars().collect();
    if needle_chars.len() > haystack_chars.len() {
        return false;
    }

    let is_word_char = |ch: char| ch.is_ascii_alphanumeric() || ch == '_';
    let first_is_word = needle_chars.first().copied().is_some_and(is_word_char);
    let last_is_word = needle_chars.last().copied().is_some_and(is_word_char);

    for start in 0..=(haystack_chars.len() - needle_chars.len()) {
        let matched = haystack_chars[start..start + needle_chars.len()]
            .iter()
            .zip(&needle_chars)
            .all(|(a, b)| a.eq_ignore_ascii_case(b));
        if !matched {
            continue;
        }
        let before_ok = !first_is_word
            || start == 0
            || !is_word_char(haystack_chars[start - 1]);
        let after_idx = start + needle_chars.len();
        let after_ok = !last_is_word
            || after_idx == haystack_chars.len()
            || !is_word_char(haystack_chars[after_idx]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x3040..=0x309F
            | 0x30A0..=0x30FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xF900..=0xFAFF
            | 0xAC00..=0xD7AF
    )
}

fn extract_leaked_cjk_run(source_text: &str, translation_text: &str) -> Option<String> {
    let mut current = String::new();
    for ch in translation_text.chars() {
        if is_cjk_char(ch) {
            current.push(ch);
        } else {
            if current.chars().count() >= 2 && source_text.contains(&current) {
                return Some(current);
            }
            current.clear();
        }
    }
    if current.chars().count() >= 2 && source_text.contains(&current) {
        return Some(current);
    }
    None
}

fn find_honorific_marker(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    for &marker in HONORIFIC_MARKERS {
        if marker.starts_with('-') {
            if lower.contains(marker) {
                return Some(marker.to_owned());
            }
        } else if text.contains(marker) {
            return Some(marker.to_owned());
        }
    }
    None
}

pub fn lint_project_glossary(
    scene: &Snapshot,
    chapter_filter: Option<&str>,
) -> Vec<GlossaryLintIssue> {
    let Some(glossary) = scene.project_component::<Glossary>().ok().flatten() else {
        return Vec::new();
    };
    if !glossary.enabled {
        return Vec::new();
    }

    let is_cjk_target = glossary
        .target_language
        .as_ref()
        .is_some_and(|lang| {
            let tag = lang.as_str().to_ascii_lowercase();
            tag.starts_with("ja") || tag.starts_with("zh") || tag.starts_with("ko")
        });

    let enabled_entries: Vec<_> = glossary
        .entries
        .iter()
        .filter(|entry| entry.enabled)
        .collect();

    let mut bubbles = Vec::new();
    for (page_index, page_ref) in scene.pages().enumerate() {
        let Ok(page_meta) = page_ref.page() else {
            continue;
        };
        if let Some(filter) = chapter_filter
            && page_meta.chapter.as_deref() != Some(filter)
        {
            continue;
        }
        let layers = ordered_page_text_layers(scene, page_ref.id());
        for layer in layers {
            let Ok(content) = layer.content() else {
                continue;
            };
            let translation_text = content
                .translation()
                .ok()
                .flatten()
                .map(|t| t.text.value)
                .unwrap_or_default();
            if translation_text.trim().is_empty() {
                continue;
            }
            let source_text = content
                .source()
                .ok()
                .flatten()
                .map(|s| s.text.value)
                .unwrap_or_default();
            let speaker_id = layer
                .speaker()
                .ok()
                .flatten()
                .and_then(|s| s.speaker)
                .or_else(|| content.speaker().ok().flatten().and_then(|s| s.speaker));

            bubbles.push(BubbleContext {
                page_id: page_ref.id(),
                page_index,
                page_label: page_meta.label.clone(),
                chapter: page_meta.chapter.clone(),
                layer_id: layer.id(),
                source_text,
                translation_text,
                speaker_id,
            });
        }
    }

    let mut issues = Vec::new();

    for bubble in &bubbles {
        let lower_translation = bubble.translation_text.to_lowercase();

        for entry in &enabled_entries {
            let Some(expected_translation) = entry
                .translation
                .as_deref()
                .map(str::trim)
                .filter(|t| !t.is_empty())
            else {
                continue;
            };

            if !entry.source.trim().is_empty() && bubble.source_text.contains(entry.source.trim()) {
                let lower_expected = expected_translation.to_lowercase();
                if !lower_translation.contains(&lower_expected) {
                    issues.push(GlossaryLintIssue {
                        kind: GlossaryLintKind::MissingGlossaryTerm,
                        severity: GlossaryLintSeverity::Warning,
                        page_id: bubble.page_id,
                        page_index: bubble.page_index,
                        page_label: bubble.page_label.clone(),
                        chapter: bubble.chapter.clone(),
                        layer_id: bubble.layer_id,
                        entry_id: Some(entry.id),
                        speaker_id: bubble.speaker_id,
                        expected: expected_translation.to_owned(),
                        actual: bubble.translation_text.clone(),
                        message: format!(
                            "Source contains '{}' but translation is missing '{}'",
                            entry.source, expected_translation
                        ),
                    });
                }
            }

            for variant in &entry.forbidden_variants {
                let trimmed_variant = variant.trim();
                if trimmed_variant.is_empty() {
                    continue;
                }
                if lower_translation.contains(&trimmed_variant.to_lowercase()) {
                    issues.push(GlossaryLintIssue {
                        kind: GlossaryLintKind::ForbiddenVariantUsed,
                        severity: GlossaryLintSeverity::Error,
                        page_id: bubble.page_id,
                        page_index: bubble.page_index,
                        page_label: bubble.page_label.clone(),
                        chapter: bubble.chapter.clone(),
                        layer_id: bubble.layer_id,
                        entry_id: Some(entry.id),
                        speaker_id: bubble.speaker_id,
                        expected: expected_translation.to_owned(),
                        actual: trimmed_variant.to_owned(),
                        message: format!(
                            "Forbidden variant '{}' used instead of '{}'",
                            trimmed_variant, expected_translation
                        ),
                    });
                }
            }
        }

        if !is_cjk_target
            && let Some(leaked) =
                extract_leaked_cjk_run(&bubble.source_text, &bubble.translation_text)
        {
            issues.push(GlossaryLintIssue {
                kind: GlossaryLintKind::UntranslatedSourceLeak,
                severity: GlossaryLintSeverity::Error,
                page_id: bubble.page_id,
                page_index: bubble.page_index,
                page_label: bubble.page_label.clone(),
                chapter: bubble.chapter.clone(),
                layer_id: bubble.layer_id,
                entry_id: None,
                speaker_id: bubble.speaker_id,
                expected: String::new(),
                actual: leaked.clone(),
                message: format!("Untranslated source text '{}' leaked into translation", leaked),
            });
        }

        if let Some(speaker_id) = bubble.speaker_id
            && let Some(speaker_entry) = enabled_entries.iter().find(|e| e.id == speaker_id)
            && let Some(character) = &speaker_entry.character
        {
            let speaker_name = speaker_entry
                .translation
                .as_deref()
                .unwrap_or(&speaker_entry.source);

            if let Some(first_person) = character
                .first_person
                .as_deref()
                .map(str::trim)
                .filter(|fp| !fp.is_empty())
            {
                let matches_candidate = TARGET_FIRST_PERSON_CANDIDATES
                    .iter()
                    .any(|cand| cand.eq_ignore_ascii_case(first_person) || *cand == first_person);
                if matches_candidate
                    && !contains_case_insensitive_whole_word(&bubble.translation_text, first_person)
                {
                    let conflicting = TARGET_FIRST_PERSON_CANDIDATES
                        .iter()
                        .copied()
                        .find(|cand| {
                            !cand.eq_ignore_ascii_case(first_person)
                                && *cand != first_person
                                && contains_case_insensitive_whole_word(
                                    &bubble.translation_text,
                                    cand,
                                )
                        });
                    if let Some(actual_pronoun) = conflicting {
                        issues.push(GlossaryLintIssue {
                            kind: GlossaryLintKind::SpeakerFirstPersonMismatch,
                            severity: GlossaryLintSeverity::Warning,
                            page_id: bubble.page_id,
                            page_index: bubble.page_index,
                            page_label: bubble.page_label.clone(),
                            chapter: bubble.chapter.clone(),
                            layer_id: bubble.layer_id,
                            entry_id: Some(speaker_entry.id),
                            speaker_id: Some(speaker_id),
                            expected: first_person.to_owned(),
                            actual: actual_pronoun.to_owned(),
                            message: format!(
                                "Speaker '{}' expects first-person '{}', found '{}'",
                                speaker_name, first_person, actual_pronoun
                            ),
                        });
                    }
                }
            }

            if let Some(policy) = character
                .honorific_policy
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty())
                && policy.eq_ignore_ascii_case("drop")
                && let Some(marker) = find_honorific_marker(&bubble.translation_text)
            {
                issues.push(GlossaryLintIssue {
                    kind: GlossaryLintKind::SpeakerHonorificConflict,
                    severity: GlossaryLintSeverity::Warning,
                    page_id: bubble.page_id,
                    page_index: bubble.page_index,
                    page_label: bubble.page_label.clone(),
                    chapter: bubble.chapter.clone(),
                    layer_id: bubble.layer_id,
                    entry_id: Some(speaker_entry.id),
                    speaker_id: Some(speaker_id),
                    expected: "drop".to_owned(),
                    actual: marker.clone(),
                    message: format!(
                        "Speaker '{}' has honorific policy 'drop' but translation contains '{}'",
                        speaker_name, marker
                    ),
                });
            }
        }
    }

    for entry in &enabled_entries {
        if entry.kind != GlossaryKind::Person {
            continue;
        }
        let speaker_bubbles: Vec<_> = bubbles
            .iter()
            .filter(|b| b.speaker_id == Some(entry.id) && b.chapter.is_some())
            .collect();
        if speaker_bubbles.is_empty() {
            continue;
        }

        let speaker_name = entry.translation.as_deref().unwrap_or(&entry.source);
        let mut chapters_with_honorifics: BTreeMap<String, Vec<&BubbleContext>> = BTreeMap::new();
        let mut chapters_without_honorifics: BTreeSet<String> = BTreeSet::new();

        let mut bubbles_by_chapter: BTreeMap<String, Vec<&BubbleContext>> = BTreeMap::new();
        for bubble in speaker_bubbles {
            if let Some(ch) = &bubble.chapter {
                bubbles_by_chapter
                    .entry(ch.clone())
                    .or_default()
                    .push(bubble);
            }
        }
        if bubbles_by_chapter.len() < 2 {
            continue;
        }

        for (ch, ch_bubbles) in &bubbles_by_chapter {
            let with_hon: Vec<&BubbleContext> = ch_bubbles
                .iter()
                .copied()
                .filter(|b| find_honorific_marker(&b.translation_text).is_some())
                .collect();
            if with_hon.is_empty() {
                chapters_without_honorifics.insert(ch.clone());
            } else {
                chapters_with_honorifics.insert(ch.clone(), with_hon);
            }
        }

        if !chapters_with_honorifics.is_empty()
            && !chapters_without_honorifics.is_empty()
            && entry
                .character
                .as_ref()
                .and_then(|c| c.honorific_policy.as_deref())
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .is_none()
            && let Some((first_hon_chapter, hon_bubbles)) = chapters_with_honorifics.iter().next()
            && let Some(first_drop_chapter) = chapters_without_honorifics.iter().next()
            && let Some(target_bubble) = hon_bubbles.first()
        {
            let marker = find_honorific_marker(&target_bubble.translation_text)
                .unwrap_or_else(|| "honorific".to_owned());
            issues.push(GlossaryLintIssue {
                kind: GlossaryLintKind::CrossChapterSpeakerDrift,
                severity: GlossaryLintSeverity::Warning,
                page_id: target_bubble.page_id,
                page_index: target_bubble.page_index,
                page_label: target_bubble.page_label.clone(),
                chapter: target_bubble.chapter.clone(),
                layer_id: target_bubble.layer_id,
                entry_id: Some(entry.id),
                speaker_id: Some(entry.id),
                expected: format!("consistent honorific usage with chapter '{}'", first_drop_chapter),
                actual: marker,
                message: format!(
                    "Speaker '{}' uses honorifics in chapter '{}' but drops them in chapter '{}'",
                    speaker_name, first_hon_chapter, first_drop_chapter
                ),
            });
        }
    }

    issues
}

#[cfg(test)]
mod tests {
    use koharu_scene::{
        At, Authored, CharacterGender, CharacterProfile, Glossary, GlossaryEntry, GlossaryEntryId,
        GlossaryKind, GlossaryValueOrigin, LanguageTag, Origin, PageDraft, Session, SourceText,
        TextLayout, TextLayoutKind, Translation,
    };

    use super::{GlossaryLintKind, GlossaryLintSeverity, lint_project_glossary};

    #[tokio::test]
    async fn lint_project_glossary_detects_all_six_issue_kinds_across_chapters() {
        let mut session = Session::memory().await.unwrap();
        let haruka_id = GlossaryEntryId::new();
        let ken_id = GlossaryEntryId::new();
        let academy_id = GlossaryEntryId::new();

        let glossary = Glossary {
            enabled: true,
            source_language: Some(LanguageTag::new("ja").unwrap()),
            target_language: Some(LanguageTag::new("en").unwrap()),
            source_fingerprint: None,
            entries: vec![
                GlossaryEntry {
                    id: haruka_id,
                    source: "春香".to_owned(),
                    translation: Some("Haruka".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    confidence: Some(0.99),
                    occurrence_count: 3,
                    examples: Vec::new(),
                    source_origin: GlossaryValueOrigin::User,
                    translation_origin: Some(GlossaryValueOrigin::User),
                    present_in_last_scan: true,
                    character: Some(CharacterProfile {
                        id: haruka_id,
                        gender: CharacterGender::Female,
                        first_person: Some("I".to_owned()),
                        second_person: None,
                        tone: None,
                        custom_instructions: None,
                        honorific_policy: Some("drop".to_owned()),
                    }),
                    forbidden_variants: vec!["Haru-chan".to_owned()],
                },
                GlossaryEntry {
                    id: ken_id,
                    source: "健".to_owned(),
                    translation: Some("Ken".to_owned()),
                    kind: GlossaryKind::Person,
                    enabled: true,
                    confidence: Some(0.95),
                    occurrence_count: 2,
                    examples: Vec::new(),
                    source_origin: GlossaryValueOrigin::User,
                    translation_origin: Some(GlossaryValueOrigin::User),
                    present_in_last_scan: true,
                    character: Some(CharacterProfile {
                        id: ken_id,
                        gender: CharacterGender::Male,
                        first_person: None,
                        second_person: None,
                        tone: None,
                        custom_instructions: None,
                        honorific_policy: None,
                    }),
                    forbidden_variants: Vec::new(),
                },
                GlossaryEntry {
                    id: academy_id,
                    source: "月影学園".to_owned(),
                    translation: Some("Moonlight Academy".to_owned()),
                    kind: GlossaryKind::Organization,
                    enabled: true,
                    confidence: Some(0.9),
                    occurrence_count: 1,
                    examples: Vec::new(),
                    source_origin: GlossaryValueOrigin::User,
                    translation_origin: Some(GlossaryValueOrigin::User),
                    present_in_last_scan: true,
                    character: None,
                    forbidden_variants: Vec::new(),
                },
            ],
        };

        let patch = session
            .snapshot()
            .patch(|edit| {
                edit.set_project(&glossary)?;

                let mut p1_draft = PageDraft::new("Chapter 1 - Page 1", 800.0, 1200.0);
                p1_draft.chapter = Some("ch-1".to_owned());
                let page1 = edit.add_page(p1_draft, At::End)?;

                let c1 = edit.add_text_content(page1, At::End)?;
                edit.set(
                    c1,
                    &SourceText {
                        text: Authored::user("春香は月影学園へ行く".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                edit.set(
                    c1,
                    &Translation {
                        text: Authored::user(
                            "We saw Haru-chan at school-san near 月影学園".to_owned(),
                        ),
                        language: Some(LanguageTag::new("en")?),
                    },
                )?;
                let l1 = edit.add_text_layer(
                    page1,
                    At::End,
                    c1,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(l1, Some(haruka_id), Origin::User)?;

                let c2 = edit.add_text_content(page1, At::End)?;
                edit.set(
                    c2,
                    &SourceText {
                        text: Authored::user("健の挨拶".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                edit.set(
                    c2,
                    &Translation {
                        text: Authored::user("Hello Haruka-san, Ken is here.".to_owned()),
                        language: Some(LanguageTag::new("en")?),
                    },
                )?;
                let l2 = edit.add_text_layer(
                    page1,
                    At::End,
                    c2,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(l2, Some(ken_id), Origin::User)?;

                let mut p2_draft = PageDraft::new("Chapter 2 - Page 1", 800.0, 1200.0);
                p2_draft.chapter = Some("ch-2".to_owned());
                let page2 = edit.add_page(p2_draft, At::End)?;

                let c3 = edit.add_text_content(page2, At::End)?;
                edit.set(
                    c3,
                    &SourceText {
                        text: Authored::user("健の返事".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                edit.set(
                    c3,
                    &Translation {
                        text: Authored::user("Hello Haruka, Ken is back.".to_owned()),
                        language: Some(LanguageTag::new("en")?),
                    },
                )?;
                let l3 = edit.add_text_layer(
                    page2,
                    At::End,
                    c3,
                    &TextLayout {
                        origin: Origin::User,
                        kind: TextLayoutKind::Paragraph,
                        angle_degrees: None,
                    },
                )?;
                edit.set_speaker(l3, Some(ken_id), Origin::User)?;

                Ok(())
            })
            .unwrap();

        let snapshot = session.commit(patch).await.unwrap().snapshot;
        let issues = lint_project_glossary(&snapshot, None);

        let kinds: Vec<_> = issues.iter().map(|i| i.kind).collect();
        assert!(kinds.contains(&GlossaryLintKind::MissingGlossaryTerm));
        assert!(kinds.contains(&GlossaryLintKind::ForbiddenVariantUsed));
        assert!(kinds.contains(&GlossaryLintKind::UntranslatedSourceLeak));
        assert!(kinds.contains(&GlossaryLintKind::SpeakerFirstPersonMismatch));
        assert!(kinds.contains(&GlossaryLintKind::SpeakerHonorificConflict));
        assert!(kinds.contains(&GlossaryLintKind::CrossChapterSpeakerDrift));

        let forbidden = issues
            .iter()
            .find(|i| i.kind == GlossaryLintKind::ForbiddenVariantUsed)
            .unwrap();
        assert_eq!(forbidden.severity, GlossaryLintSeverity::Error);
        assert_eq!(forbidden.expected, "Haruka");
        assert_eq!(forbidden.actual, "Haru-chan");

        let leak = issues
            .iter()
            .find(|i| i.kind == GlossaryLintKind::UntranslatedSourceLeak)
            .unwrap();
        assert_eq!(leak.severity, GlossaryLintSeverity::Error);
        assert_eq!(leak.actual, "月影学園");

        let ch2_issues = lint_project_glossary(&snapshot, Some("ch-2"));
        assert!(ch2_issues.is_empty());
    }
}
