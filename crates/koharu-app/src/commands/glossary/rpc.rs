use anyhow::Context as _;
use koharu_scene::{GlossaryEntryId, Revision};
use tauri::State;

use super::{
    GlossaryEntryDraft, GlossaryEntryPatch, GlossaryImportPreview, GlossaryImportStrategy,
    GlossaryView,
};
use crate::commands::{
    ChannelExt as _, Error, lifecycle::ProjectChannel, project::CurrentProject,
};

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_glossary(
    project: State<'_, CurrentProject>,
) -> Result<GlossaryView, Error> {
    let current = project.project.lock().await;
    let project = current.as_ref().context("no project is open")?;
    Ok(project.glossary_view()?)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn set_glossary_enabled(
    expected_revision: Revision,
    enabled: bool,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let view = project
            .set_glossary_enabled(expected_revision, enabled)
            .await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn add_glossary_entry(
    expected_revision: Revision,
    draft: GlossaryEntryDraft,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let view = project.add_glossary_entry(expected_revision, draft).await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn update_glossary_entry(
    expected_revision: Revision,
    id: GlossaryEntryId,
    patch: GlossaryEntryPatch,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let view = project
            .update_glossary_entry(expected_revision, id, patch)
            .await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_glossary_entries(
    expected_revision: Revision,
    ids: Vec<GlossaryEntryId>,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let view = project
            .delete_glossary_entries(expected_revision, ids)
            .await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn preview_glossary_import(
    document: String,
    project: State<'_, CurrentProject>,
) -> Result<GlossaryImportPreview, Error> {
    let current = project.project.lock().await;
    let project = current.as_ref().context("no project is open")?;
    Ok(project.preview_glossary_import(&document)?)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn apply_glossary_import(
    expected_revision: Revision,
    document: String,
    strategy: GlossaryImportStrategy,
    confirm_language_mismatch: bool,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let view = project
            .apply_glossary_import(
                expected_revision,
                &document,
                strategy,
                confirm_language_mismatch,
            )
            .await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn export_glossary(
    project: State<'_, CurrentProject>,
) -> Result<String, Error> {
    let current = project.project.lock().await;
    let project = current.as_ref().context("no project is open")?;
    Ok(project.export_glossary_json()?)
}

pub(crate) fn extract_project_scan_candidates(
    snapshot: &koharu_scene::Snapshot,
) -> anyhow::Result<Vec<super::domain::ScanCandidate>> {
    use super::common_words::{has_repeated_letters, is_common_english_word, is_laugh_or_noise};
    use koharu_scene::{GlossaryKind, SourceText};
    use std::collections::HashSet;

    const LATIN_HONORIFICS: &[&str] = &[
        "-san", "-chan", "-kun", "-sama", "-senpai", "-sensei", "-dono", "-tan", "-shi",
    ];
    const KATA_HONORIFICS: &[&str] = &["サン", "チャン", "クン", "サマ", "センパイ", "センセイ"];

    let mut candidates = Vec::new();
    let mut seen = HashSet::new();

    for page in snapshot.pages() {
        let Ok(subtree) = snapshot.subtree(page.id()) else {
            continue;
        };
        for entity in subtree {
            let Ok(Some(source)) = snapshot.component::<SourceText>(entity.id()) else {
                continue;
            };
            let text = source.text.value.trim();
            if text.is_empty() {
                continue;
            }

            // 1. Quoted terms: 「...」, 『...』, 【...】
            let mut in_quote = false;
            let mut quote_buf = String::new();
            for ch in text.chars() {
                match ch {
                    '「' | '『' | '【' => {
                        in_quote = true;
                        quote_buf.clear();
                    }
                    '」' | '』' | '】' => {
                        in_quote = false;
                        let term = quote_buf.trim();
                        if term.chars().count() >= 2 && term.chars().count() <= 20 {
                            if seen.insert((term.to_string(), GlossaryKind::Term)) {
                                candidates.push(super::domain::ScanCandidate {
                                    source: term.to_string(),
                                    kind: GlossaryKind::Term,
                                    confidence: 0.90,
                                    example: Some(text.chars().take(80).collect()),
                                });
                            }
                        }
                    }
                    _ => {
                        if in_quote {
                            quote_buf.push(ch);
                            if quote_buf.chars().count() > 30 {
                                in_quote = false;
                            }
                        }
                    }
                }
            }

            // 2. Katakana sequences (2 to 16 characters)
            let mut kata_buf = String::new();
            let mut flush_kata = |buf: &mut String| {
                let term = buf.trim();
                let char_count = term.chars().count();
                if char_count >= 2 && char_count <= 16 {
                    let chars: Vec<char> = term.chars().collect();
                    let is_repeated = chars.len() >= 4
                        && chars.len() % 2 == 0
                        && chars[..chars.len() / 2] == chars[chars.len() / 2..];
                    let is_scream = chars.len() >= 3 && {
                        let first = chars[0];
                        chars.iter().all(|&c| c == first || c == 'ー' || c == 'ッ')
                    };
                    if !is_repeated && !is_scream {
                        let mut base_term = term;
                        for &h in KATA_HONORIFICS {
                            if let Some(stripped) = term.strip_suffix(h) {
                                if stripped.chars().count() >= 2 {
                                    base_term = stripped;
                                    break;
                                }
                            }
                        }
                        if seen.insert((base_term.to_string(), GlossaryKind::Person)) {
                            candidates.push(super::domain::ScanCandidate {
                                source: base_term.to_string(),
                                kind: GlossaryKind::Person,
                                confidence: 0.85,
                                example: Some(text.chars().take(80).collect()),
                            });
                        }
                    }
                }
                buf.clear();
            };

            for ch in text.chars() {
                if ('\u{30A0}'..='\u{30FF}').contains(&ch) || ch == '\u{30FC}' {
                    kata_buf.push(ch);
                } else {
                    flush_kata(&mut kata_buf);
                }
            }
            flush_kata(&mut kata_buf);

            // 3. Latin words (English / Romaji)
            // Dialogue in comics/manga is often in ALL CAPS.
            let total_ascii_alpha = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
            let upper_ascii_alpha = text.chars().filter(|c| c.is_ascii_uppercase()).count();
            let is_all_caps_block = total_ascii_alpha >= 3
                && (upper_ascii_alpha as f64 / total_ascii_alpha as f64) >= 0.7;

            for word in text.split_whitespace() {
                let cleaned = word.trim_matches(|c: char| !c.is_ascii_alphabetic() && c != '-');
                if cleaned.is_empty() {
                    continue;
                }

                // Check Latin honorifics like Isuzu-san, ISUZU-SAN
                let lower = cleaned.to_lowercase();
                let mut found_honorific = false;
                for &h in LATIN_HONORIFICS {
                    if lower.ends_with(h) {
                        let base_len = cleaned.len() - h.len();
                        let base = cleaned[..base_len]
                            .trim_matches(|c: char| !c.is_ascii_alphabetic());
                        let base_lower = base.to_lowercase();
                        if base.chars().count() >= 2 && !is_common_english_word(&base_lower) {
                            if seen.insert((base.to_string(), GlossaryKind::Person)) {
                                candidates.push(super::domain::ScanCandidate {
                                    source: base.to_string(),
                                    kind: GlossaryKind::Person,
                                    confidence: 0.95,
                                    example: Some(text.chars().take(80).collect()),
                                });
                            }
                            found_honorific = true;
                            break;
                        }
                    }
                }
                if found_honorific {
                    continue;
                }

                let cleaned_alpha = cleaned.trim_matches(|c: char| !c.is_ascii_alphabetic());
                let char_count = cleaned_alpha.chars().count();
                if char_count < 3 || char_count > 20 {
                    continue;
                }

                let lower_alpha = cleaned_alpha.to_lowercase();
                if is_common_english_word(&lower_alpha)
                    || has_repeated_letters(&lower_alpha)
                    || is_laugh_or_noise(&lower_alpha)
                {
                    continue;
                }

                if is_all_caps_block {
                    if seen.insert((cleaned_alpha.to_string(), GlossaryKind::Person)) {
                        candidates.push(super::domain::ScanCandidate {
                            source: cleaned_alpha.to_string(),
                            kind: GlossaryKind::Person,
                            confidence: 0.85,
                            example: Some(text.chars().take(80).collect()),
                        });
                    }
                } else {
                    let mut chars = cleaned_alpha.chars();
                    let is_title_case = chars.next().is_some_and(|c| c.is_ascii_uppercase())
                        && chars.all(|c| c.is_ascii_lowercase());
                    if is_title_case
                        && seen.insert((cleaned_alpha.to_string(), GlossaryKind::Person))
                    {
                        candidates.push(super::domain::ScanCandidate {
                            source: cleaned_alpha.to_string(),
                            kind: GlossaryKind::Person,
                            confidence: 0.85,
                            example: Some(text.chars().take(80).collect()),
                        });
                    }
                }
            }
        }
    }

    Ok(candidates)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn scan_glossary(
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let candidates = extract_project_scan_candidates(&project.snapshot())?;
        let revision = project.revision();
        let view = project
            .apply_glossary_scan(revision, None, None, candidates)
            .await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn translate_glossary_entries(
    expected_revision: Revision,
    ids: Option<Vec<GlossaryEntryId>>,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let target_lang = project
            .glossary_view()?
            .target_language
            .map(|l| l.to_string())
            .unwrap_or_else(|| "en".to_string());

        let target_ids = ids.unwrap_or_default();
        let current_view = project.glossary_view()?;
        let client = reqwest::Client::new();
        let mut results = Vec::new();

        for entry in &current_view.entries {
            if (!target_ids.is_empty() && !target_ids.contains(&entry.id))
                || entry.translation.is_some()
            {
                continue;
            }
            if let Ok(res) = client
                .get("https://translate.googleapis.com/translate_a/single")
                .query(&[
                    ("client", "gtx"),
                    ("sl", "auto"),
                    ("tl", &target_lang),
                    ("dt", "t"),
                    ("q", &entry.source),
                ])
                .send()
                .await
            {
                if let Ok(json) = res.json::<serde_json::Value>().await {
                    if let Some(arr) = json
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|b| b.as_array())
                    {
                        let mut trans = String::new();
                        for seg in arr {
                            if let Some(t) =
                                seg.as_array().and_then(|s| s.first()).and_then(|v| v.as_str())
                            {
                                trans.push_str(t);
                            }
                        }
                        if !trans.is_empty() {
                            results.push(super::domain::TermTranslationResult {
                                id: entry.id,
                                source: entry.source.clone(),
                                translation: trans,
                            });
                        }
                    }
                }
            }
        }

        let view = if !results.is_empty() {
            project
                .apply_term_translations(expected_revision, results)
                .await?
        } else {
            current_view
        };
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}
