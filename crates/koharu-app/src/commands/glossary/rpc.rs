use anyhow::Context as _;
use koharu_scene::{
    CharacterProfile, EntityId, GlossaryEntryId, GlossaryKind, Presents, Revision, SourceText,
};
use tauri::State;

use super::{
    GlossaryEntryDraft, GlossaryEntryPatch, GlossaryImportPreview, GlossaryImportStrategy,
    GlossaryLintIssue, GlossaryScanReport, GlossaryView, lint_project_glossary,
};
use crate::commands::{
    ChannelExt as _, Error, lifecycle::ProjectChannel, project::CurrentProject,
};

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GlossaryOccurrence {
    pub page_id: EntityId,
    pub entity_id: EntityId,
    pub page_index: u32,
    pub snippet: String,
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn locate_glossary_entry(
    entry_id: GlossaryEntryId,
    project: State<'_, CurrentProject>,
) -> Result<Vec<GlossaryOccurrence>, Error> {
    let current = project.project.lock().await;
    let project = current.as_ref().context("no project is open")?;
    let snapshot = project.snapshot();
    let glossary = super::domain::glossary(&snapshot)?;
    let entry = glossary
        .entries
        .iter()
        .find(|e| e.id == entry_id)
        .context("glossary entry not found")?;

    let source = entry.source.trim();
    if source.is_empty() {
        return Ok(Vec::new());
    }

    Ok(collect_glossary_occurrences(&snapshot, source))
}

pub(crate) fn collect_glossary_occurrences(
    snapshot: &koharu_scene::Snapshot,
    source: &str,
) -> Vec<GlossaryOccurrence> {
    let normalized_source = koharu_scene::normalize_glossary_source(source);
    if normalized_source.is_empty() {
        return Vec::new();
    }
    let mut occurrences = Vec::new();
    for (page_index, page) in snapshot.pages().enumerate() {
        let Ok(subtree) = snapshot.subtree(page.id()) else {
            continue;
        };
        for entity in subtree {
            let Ok(Some(text_comp)) = snapshot.component::<SourceText>(entity.id()) else {
                continue;
            };
            if koharu_scene::normalize_glossary_source(&text_comp.text.value)
                .contains(&normalized_source)
            {
                let clean_snippet = text_comp
                    .text
                    .value
                    .chars()
                    .map(|c| if c.is_control() { ' ' } else { c })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let presenting_layer_id = snapshot
                    .relations_to_as::<Presents>(entity.id())
                    .next()
                    .map(|r| r.value().source)
                    .unwrap_or_else(|| entity.id());
                occurrences.push(GlossaryOccurrence {
                    page_id: page.id(),
                    entity_id: presenting_layer_id,
                    page_index: page_index as u32,
                    snippet: clean_snippet,
                });
            }
        }
    }
    occurrences
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn update_character_profile(
    expected_revision: Revision,
    entry_id: GlossaryEntryId,
    profile: Option<CharacterProfile>,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let (view, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let view = project
            .update_character_profile(expected_revision, entry_id, profile)
            .await?;
        (view, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn lint_glossary(
    chapter: Option<String>,
    project: State<'_, CurrentProject>,
) -> Result<Vec<GlossaryLintIssue>, Error> {
    let current = project.project.lock().await;
    let project = current.as_ref().context("no project is open")?;
    Ok(lint_project_glossary(
        &project.snapshot(),
        chapter.as_deref(),
    ))
}

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

#[tauri::command]
#[specta::specta]
pub(crate) async fn scan_glossary(
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryScanReport, Error> {
    let target_language = {
        let pipeline = koharu_pipeline::PipelineConfig::load()?;
        let read = pipeline.read()?;
        koharu_scene::LanguageTag::new(read.translation.target_language.tag())?
    };
    let (report, info) = {
        let mut current = project.project.lock().await;
        let project = current.as_mut().context("no project is open")?;
        let snapshot = project.snapshot();
        let input = super::scan::collect_project_scan_input(&snapshot, target_language)?;
        let revision = snapshot.revision();
        let report = project.apply_glossary_scan(revision, input).await?;
        (report, project.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(report)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn translate_glossary_entries(
    expected_revision: Revision,
    ids: Option<Vec<GlossaryEntryId>>,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    let configured_target = {
        let pipeline = koharu_pipeline::PipelineConfig::load()?;
        let read = pipeline.read()?;
        read.translation.target_language.tag().to_owned()
    };
    let (target_lang, to_translate) = {
        let current = project.project.lock().await;
        let proj = current.as_ref().context("no project is open")?;
        let current_view = proj.glossary_view()?;
        let target_lang = current_view
            .target_language
            .map(|l| l.to_string())
            .unwrap_or(configured_target);
        let target_ids = ids.unwrap_or_default();
        let items: Vec<(GlossaryEntryId, String)> = current_view
            .entries
            .into_iter()
            .filter(|e| (target_ids.is_empty() || target_ids.contains(&e.id)) && e.translation.is_none())
            .map(|e| (e.id, e.source))
            .collect();
        (target_lang, items)
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let mut results = Vec::new();
    for (entry_id, source) in to_translate {
        let json = client
            .get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", "auto"),
                ("tl", &target_lang),
                ("dt", "t"),
                ("q", &source),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;
        let segments = json
            .as_array()
            .and_then(|value| value.first())
            .and_then(|value| value.as_array())
            .context("translation service returned an unexpected response")?;
        let mut translation = String::new();
        for segment in segments {
            if let Some(text) = segment
                .as_array()
                .and_then(|values| values.first())
                .and_then(serde_json::Value::as_str)
            {
                translation.push_str(text);
            }
        }
        if translation.trim().is_empty() {
            return Err(
                anyhow::anyhow!("translation service returned an empty translation").into(),
            );
        }
        results.push(super::domain::TermTranslationResult {
            id: entry_id,
            source,
            translation,
        });
    }

    let (view, info) = {
        let mut current = project.project.lock().await;
        let proj = current.as_mut().context("no project is open")?;
        let view = if !results.is_empty() {
            proj.apply_term_translations(expected_revision, results).await?
        } else {
            proj.glossary_view()?
        };
        (view, proj.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn classify_glossary_entries(
    expected_revision: Revision,
    ids: Option<Vec<GlossaryEntryId>>,
    project: State<'_, CurrentProject>,
    project_channel: State<'_, ProjectChannel>,
) -> Result<GlossaryView, Error> {
    use super::domain::{JP_HONORIFICS, KATA_HONORIFICS, LATIN_HONORIFICS};

    let entries_to_classify = {
        let current = project.project.lock().await;
        let proj = current.as_ref().context("no project is open")?;
        let current_view = proj.glossary_view()?;
        let target_ids = ids.unwrap_or_default();
        current_view
            .entries
            .into_iter()
            .filter(|e| target_ids.is_empty() || target_ids.contains(&e.id))
            .collect::<Vec<_>>()
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let mut updates = Vec::new();
    for entry in entries_to_classify {
        let source = entry.source.trim();
        if source.is_empty() {
            continue;
        }

        let lower = source.to_lowercase();
        let has_jp_honorific = JP_HONORIFICS.iter().any(|&h| source.ends_with(h));
        let has_kata_honorific = KATA_HONORIFICS.iter().any(|&h| source.ends_with(h));
        let has_latin_honorific = LATIN_HONORIFICS.iter().any(|&h| lower.ends_with(h));
        if has_jp_honorific || has_kata_honorific || has_latin_honorific {
            if entry.kind != GlossaryKind::Person {
                updates.push((entry.id, GlossaryKind::Person));
            }
            continue;
        }

        let is_katakana = source
            .chars()
            .all(|c| ('\u{30A0}'..='\u{30FF}').contains(&c) || c == 'ー');
        if is_katakana && source.chars().count() >= 2 && source.chars().count() <= 6 {
            if entry.kind != GlossaryKind::Person {
                updates.push((entry.id, GlossaryKind::Person));
            }
            continue;
        }

        let trans_text = if let Some(t) = &entry.translation {
            t.clone()
        } else if let Ok(res) = client
            .get("https://translate.googleapis.com/translate_a/single")
            .query(&[
                ("client", "gtx"),
                ("sl", "auto"),
                ("tl", "en"),
                ("dt", "t"),
                ("q", source),
            ])
            .send()
            .await
        {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                let mut trans = String::new();
                if let Some(arr) = json
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|b| b.as_array())
                {
                    for seg in arr {
                        if let Some(t) =
                            seg.as_array().and_then(|s| s.first()).and_then(|v| v.as_str())
                        {
                            trans.push_str(t);
                        }
                    }
                }
                trans
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let clean_trans = trans_text.trim();
        if !clean_trans.is_empty() {
            let first_char = clean_trans.chars().next().unwrap_or(' ');
            let word_count = clean_trans.split_whitespace().count();
            if first_char.is_ascii_uppercase() && word_count <= 3 {
                if entry.kind != GlossaryKind::Person {
                    updates.push((entry.id, GlossaryKind::Person));
                }
            } else if first_char.is_ascii_lowercase() {
                if entry.kind != GlossaryKind::Term {
                    updates.push((entry.id, GlossaryKind::Term));
                }
            }
        }
    }

    let (view, info) = {
        let mut current = project.project.lock().await;
        let proj = current.as_mut().context("no project is open")?;
        let view = if !updates.is_empty() {
            proj.update_entry_kinds(expected_revision, updates).await?
        } else {
            proj.glossary_view()?
        };
        (view, proj.info())
    };
    project_channel.channel.publish(Some(info));
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use koharu_scene::{At, Authored, LanguageTag, PageDraft, Session, SourceText};

    fn scan_candidates(snapshot: &koharu_scene::Snapshot) -> Vec<super::super::ScanCandidate> {
        super::super::scan::collect_project_scan_input(
            snapshot,
            LanguageTag::new("en").unwrap(),
        )
        .unwrap()
        .candidates
    }

    #[tokio::test]
    async fn scan_candidates_normalize_multiline_ocr_examples() {
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let text1 = edit.add_text_content(page, At::End)?;
                edit.set(
                    text1,
                    &SourceText {
                        text: Authored::user("「ハルカ」\nおはようございます\r\n先生！".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                let text2 = edit.add_text_content(page, At::End)?;
                edit.set(
                    text2,
                    &SourceText {
                        text: Authored::user("KATAS\nISUZU-SAN\nHELLO".to_owned()),
                        language: Some(LanguageTag::new("en")?),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        session.commit(setup).await.unwrap();

        let candidates = scan_candidates(&session.snapshot());
        assert!(!candidates.is_empty());
        for candidate in &candidates {
            assert!(candidate.validate().is_ok(), "Candidate failed validation: {:?}", candidate);
            if let Some(example) = &candidate.example {
                assert!(!example.chars().any(char::is_control));
                assert!(!example.contains('\n'));
                assert!(!example.contains('\r'));
            }
        }
    }

    #[tokio::test]
    async fn scan_extracts_japanese_honorific_names_as_person() {
        let mut session = Session::memory().await.unwrap();
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let text = edit.add_text_content(page, At::End)?;
                edit.set(
                    text,
                    &SourceText {
                        text: Authored::user("田中さん、美咲先生が呼んでるよ！".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
                Ok(())
            })
            .unwrap();
        session.commit(setup).await.unwrap();

        let candidates = scan_candidates(&session.snapshot());
        let tanaka = candidates.iter().find(|c| c.source == "田中");
        assert!(tanaka.is_some(), "Expected 田中 in candidates, found: {:?}", candidates);
        assert_eq!(tanaka.unwrap().kind, GlossaryKind::Person);

        let misaki = candidates.iter().find(|c| c.source == "美咲");
        assert!(misaki.is_some(), "Expected 美咲 in candidates, found: {:?}", candidates);
        assert_eq!(misaki.unwrap().kind, GlossaryKind::Person);
    }

    #[tokio::test]
    async fn locate_glossary_entry_returns_presenting_text_layer_id() {
        use koharu_scene::{Origin, TextLayout, TextLayoutKind};

        let mut session = Session::memory().await.unwrap();
        let mut expected_layer = None;
        let setup = session
            .snapshot()
            .patch(|edit| {
                let page = edit.add_page(PageDraft::new("p1", 100.0, 100.0), At::End)?;
                let content = edit.add_text_content(page, At::End)?;
                edit.set(
                    content,
                    &SourceText {
                        text: Authored::user("アリスは元気です".to_owned()),
                        language: Some(LanguageTag::new("ja")?),
                    },
                )?;
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
                expected_layer = Some(layer);
                Ok(())
            })
            .unwrap();
        let snapshot = session.commit(setup).await.unwrap().snapshot;

        let occurrences = collect_glossary_occurrences(&snapshot, "アリス");
        assert_eq!(occurrences.len(), 1);
        assert_eq!(occurrences[0].entity_id, expected_layer.unwrap());
    }
}
