use std::collections::{HashMap, HashSet};
use std::ops::Range;

use anyhow::{Context as _, Result};
use koharu_scene::{GlossaryKind, LanguageTag, Snapshot, SourceText, normalize_glossary_source};

use super::common_words::{has_repeated_letters, is_common_english_word, is_laugh_or_noise};
use super::domain::{JP_HONORIFICS, KATA_HONORIFICS, LATIN_HONORIFICS, ScanCandidate};

pub(crate) struct GlossaryScanInput {
    pub source_text_count: u32,
    pub source_language: Option<LanguageTag>,
    pub target_language: LanguageTag,
    pub candidates: Vec<ScanCandidate>,
}

pub(crate) fn collect_project_scan_input(
    snapshot: &Snapshot,
    target_language: LanguageTag,
) -> Result<GlossaryScanInput> {
    let mut sources = Vec::new();
    let mut languages = HashSet::new();

    for page in snapshot.pages() {
        for entity in snapshot.subtree(page.id())? {
            let Some(source) = snapshot.component::<SourceText>(entity.id())? else {
                continue;
            };
            let text = source.text.value.trim();
            if text.is_empty() {
                continue;
            }
            if let Some(language) = source.language {
                languages.insert(language.to_string());
            }
            sources.push(text.to_owned());
        }
    }

    let source_text_count = u32::try_from(sources.len()).context("too many OCR text records")?;
    let source_language = match languages.len() {
        1 => languages
            .into_iter()
            .next()
            .map(|language| LanguageTag::new(language))
            .transpose()?,
        _ => None,
    };
    let candidates = sources
        .iter()
        .flat_map(|text| extract_source_candidates(text))
        .collect();

    Ok(GlossaryScanInput {
        source_text_count,
        source_language,
        target_language,
        candidates,
    })
}

fn extract_source_candidates(text: &str) -> Vec<ScanCandidate> {
    let example = clean_example(text);
    let mut candidates = Vec::new();
    let mut seen = HashMap::new();

    extract_quoted_terms(text, example.as_deref(), &mut candidates, &mut seen);
    extract_katakana_names(text, example.as_deref(), &mut candidates, &mut seen);
    extract_latin_names(text, example.as_deref(), &mut candidates, &mut seen);
    extract_honorific_names(text, example.as_deref(), &mut candidates, &mut seen);

    candidates
}

fn clean_example(text: &str) -> Option<String> {
    let cleaned = text
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (!cleaned.is_empty()).then(|| cleaned.chars().take(80).collect())
}

type OccurrenceKey = (String, GlossaryKind, usize, usize);

fn record_candidate(
    candidates: &mut Vec<ScanCandidate>,
    seen: &mut HashMap<OccurrenceKey, usize>,
    source: String,
    kind: GlossaryKind,
    confidence: f32,
    example: Option<&str>,
    range: Range<usize>,
) {
    let key = (
        normalize_glossary_source(&source),
        kind.clone(),
        range.start,
        range.end,
    );
    if let Some(&index) = seen.get(&key) {
        if confidence > candidates[index].confidence {
            candidates[index].confidence = confidence;
        }
        return;
    }

    seen.insert(key, candidates.len());
    candidates.push(ScanCandidate {
        source,
        kind,
        confidence,
        example: example.map(str::to_owned),
    });
}

fn extract_quoted_terms(
    text: &str,
    example: Option<&str>,
    candidates: &mut Vec<ScanCandidate>,
    seen: &mut HashMap<OccurrenceKey, usize>,
) {
    let mut quote_start = None;
    let mut quote_buf = String::new();

    for (index, character) in text.char_indices() {
        match character {
            '「' | '『' | '【' => {
                quote_start = Some(index + character.len_utf8());
                quote_buf.clear();
            }
            '」' | '』' | '】' => {
                let start = quote_start.take();
                let term = quote_buf
                    .chars()
                    .map(|character| {
                        if character.is_control() {
                            ' '
                        } else {
                            character
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Some(start) = start
                    && (2..=20).contains(&term.chars().count())
                {
                    record_candidate(
                        candidates,
                        seen,
                        term,
                        GlossaryKind::Term,
                        0.90,
                        example,
                        start..index,
                    );
                }
                quote_buf.clear();
            }
            _ if quote_start.is_some() => {
                quote_buf.push(character);
                if quote_buf.chars().count() > 30 {
                    quote_start = None;
                    quote_buf.clear();
                }
            }
            _ => {}
        }
    }
}

fn extract_katakana_names(
    text: &str,
    example: Option<&str>,
    candidates: &mut Vec<ScanCandidate>,
    seen: &mut HashMap<OccurrenceKey, usize>,
) {
    let mut buffer = String::new();
    let mut start = None;

    for (index, character) in text.char_indices() {
        if ('\u{30A0}'..='\u{30FF}').contains(&character) || character == 'ー' {
            start.get_or_insert(index);
            buffer.push(character);
        } else {
            flush_katakana(&mut buffer, start.take(), example, candidates, seen);
        }
    }
    flush_katakana(&mut buffer, start, example, candidates, seen);
}

fn flush_katakana(
    buffer: &mut String,
    start: Option<usize>,
    example: Option<&str>,
    candidates: &mut Vec<ScanCandidate>,
    seen: &mut HashMap<OccurrenceKey, usize>,
) {
    let Some(start) = start else {
        buffer.clear();
        return;
    };

    let term = buffer.trim();
    let char_count = term.chars().count();
    if (2..=16).contains(&char_count) {
        let chars = term.chars().collect::<Vec<_>>();
        let is_repeated = chars.len() >= 4
            && chars.len() % 2 == 0
            && chars[..chars.len() / 2] == chars[chars.len() / 2..];
        let is_scream = chars.len() >= 3 && {
            let first = chars[0];
            chars
                .iter()
                .all(|&character| character == first || character == 'ー' || character == 'ッ')
        };

        if !is_repeated && !is_scream {
            let base = KATA_HONORIFICS
                .iter()
                .find_map(|&honorific| term.strip_suffix(honorific))
                .filter(|base| base.chars().count() >= 2)
                .unwrap_or(term);
            record_candidate(
                candidates,
                seen,
                base.to_owned(),
                GlossaryKind::Person,
                0.85,
                example,
                start..start + base.len(),
            );
        }
    }
    buffer.clear();
}

fn extract_latin_names(
    text: &str,
    example: Option<&str>,
    candidates: &mut Vec<ScanCandidate>,
    seen: &mut HashMap<OccurrenceKey, usize>,
) {
    let total_ascii_alpha = text.chars().filter(char::is_ascii_alphabetic).count();
    let upper_ascii_alpha = text.chars().filter(char::is_ascii_uppercase).count();
    let is_all_caps_block =
        total_ascii_alpha >= 3 && (upper_ascii_alpha as f64 / total_ascii_alpha as f64) >= 0.7;

    for range in word_ranges(text) {
        let word = &text[range.clone()];
        let cleaned = word
            .trim_matches(|character: char| !character.is_ascii_alphabetic() && character != '-');
        if cleaned.is_empty() {
            continue;
        }
        let cleaned_start = range.start + word.find(cleaned).unwrap_or(0);

        let lower = cleaned.to_lowercase();
        let mut found_honorific = false;
        for &honorific in LATIN_HONORIFICS {
            if lower.ends_with(honorific) {
                let base_len = cleaned.len() - honorific.len();
                let base = cleaned[..base_len]
                    .trim_matches(|character: char| !character.is_ascii_alphabetic());
                let base_lower = base.to_lowercase();
                if base.chars().count() >= 2 && !is_common_english_word(&base_lower) {
                    let base_start = cleaned_start + cleaned[..base_len].find(base).unwrap_or(0);
                    record_candidate(
                        candidates,
                        seen,
                        base.to_owned(),
                        GlossaryKind::Person,
                        0.95,
                        example,
                        base_start..base_start + base.len(),
                    );
                    found_honorific = true;
                    break;
                }
            }
        }
        if found_honorific {
            continue;
        }

        let cleaned_alpha =
            cleaned.trim_matches(|character: char| !character.is_ascii_alphabetic());
        let char_count = cleaned_alpha.chars().count();
        if !(3..=20).contains(&char_count) {
            continue;
        }
        let lower_alpha = cleaned_alpha.to_lowercase();
        if is_common_english_word(&lower_alpha)
            || has_repeated_letters(&lower_alpha)
            || is_laugh_or_noise(&lower_alpha)
        {
            continue;
        }

        let is_title_case = {
            let mut chars = cleaned_alpha.chars();
            chars
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
                && chars.all(|character| character.is_ascii_lowercase())
        };
        if is_all_caps_block || is_title_case {
            let start = cleaned_start + cleaned.find(cleaned_alpha).unwrap_or(0);
            record_candidate(
                candidates,
                seen,
                cleaned_alpha.to_owned(),
                GlossaryKind::Person,
                0.85,
                example,
                start..start + cleaned_alpha.len(),
            );
        }
    }
}

fn word_ranges(text: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_whitespace() {
            if let Some(start) = start.take() {
                ranges.push(start..index);
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(start) = start {
        ranges.push(start..text.len());
    }
    ranges
}

fn extract_honorific_names(
    text: &str,
    example: Option<&str>,
    candidates: &mut Vec<ScanCandidate>,
    seen: &mut HashMap<OccurrenceKey, usize>,
) {
    for &honorific in JP_HONORIFICS {
        let mut search_start = 0;
        while let Some(relative_index) = text[search_start..].find(honorific) {
            let honorific_start = search_start + relative_index;
            search_start = honorific_start + honorific.len();

            let mut name_start = honorific_start;
            let mut name_chars = Vec::new();
            while let Some((index, character)) = text[..name_start].char_indices().next_back() {
                if ('\u{4E00}'..='\u{9FFF}').contains(&character)
                    || ('\u{30A0}'..='\u{30FF}').contains(&character)
                    || character == '々'
                    || character == 'ー'
                {
                    name_start = index;
                    name_chars.push(character);
                    if name_chars.len() >= 6 {
                        break;
                    }
                } else {
                    break;
                }
            }

            if name_chars.is_empty() {
                continue;
            }
            name_chars.reverse();
            let name = name_chars.into_iter().collect::<String>();
            if (1..=5).contains(&name.chars().count()) {
                record_candidate(
                    candidates,
                    seen,
                    name,
                    GlossaryKind::Person,
                    0.95,
                    example,
                    name_start..honorific_start,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koharu_scene::{At, Authored, PageDraft, Session};

    #[tokio::test]
    async fn scan_input_counts_nonempty_source_text_and_uses_record_language() {
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let empty = edit.add_text_content(page, At::End)?;
                edit.set(
                    empty,
                    &SourceText {
                        text: Authored::user(" \n ".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                let text = edit.add_text_content(page, At::End)?;
                edit.set(
                    text,
                    &SourceText {
                        text: Authored::user("田中さん、田中さん！".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                let repeated = edit.add_text_content(page, At::End)?;
                edit.set(
                    repeated,
                    &SourceText {
                        text: Authored::user("田中さん".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(setup).await.unwrap().snapshot;

        let input = collect_project_scan_input(&snapshot, LanguageTag::new("fr").unwrap()).unwrap();

        assert_eq!(input.source_text_count, 2);
        assert_eq!(input.source_language.unwrap().as_str(), "ja");
        assert_eq!(input.target_language.as_str(), "fr");
        let tanaka = input
            .candidates
            .iter()
            .filter(|candidate| candidate.source == "田中")
            .count();
        assert_eq!(tanaka, 3);
    }

    #[tokio::test]
    async fn overlapping_katakana_and_honorific_rules_count_one_occurrence() {
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let text = edit.add_text_content(page, At::End)?;
                edit.set(
                    text,
                    &SourceText {
                        text: Authored::user("アリスさん".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(setup).await.unwrap().snapshot;

        let input = collect_project_scan_input(&snapshot, LanguageTag::new("en").unwrap()).unwrap();

        assert_eq!(
            input
                .candidates
                .iter()
                .filter(|candidate| candidate.source == "アリス")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn scan_examples_are_single_line_and_valid() {
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let text = edit.add_text_content(page, At::End)?;
                edit.set(
                    text,
                    &SourceText {
                        text: Authored::user("「ハルカ」\nISUZU-SAN\r\nHELLO".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(setup).await.unwrap().snapshot;

        let input = collect_project_scan_input(&snapshot, LanguageTag::new("en").unwrap()).unwrap();

        assert!(!input.candidates.is_empty());
        for candidate in &input.candidates {
            assert!(candidate.validate().is_ok());
            assert!(
                candidate
                    .example
                    .as_deref()
                    .is_none_or(|example| !example.chars().any(char::is_control))
            );
        }
    }
}
