use anyhow::{Context as _, Result};
use koharu_desktop::{CanvasState, Desktop};
use koharu_scene::{AssetInput, AssetMetadata, AssetRole, At, PageDraft};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use specta::Type;
use strum::{EnumMessage as _, IntoEnumIterator as _};
use tauri::{AppHandle, Manager as _, State, WebviewWindow, ipc::Channel};
use tauri_runtime_cef::CefRuntime;
use walkdir::WalkDir;

use super::{
    ChannelExt as _, Error,
    agent::AgentState,
    canvas::CanvasChannel,
    import,
    preferences::Preferences,
    processing::{Job, JobChannel, Processing},
    project::{
        CurrentProject, Page, PageSummary, Project, ProjectInfo, ProjectLibrary, ProjectSummary,
    },
};

#[derive(Clone, Debug, Serialize, Type)]
pub struct StartupState {
    pub preferences: Preferences,
    pub jobs: Vec<Job>,
    pub canvas: CanvasState,
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct PageSelection {
    pub project: ProjectInfo,
    pub page: Page,
}

#[derive(Clone, Copy, Debug, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PageImportSource {
    Files,
    Folder,
}

pub(crate) struct Initialization {
    ready: tokio::sync::watch::Sender<bool>,
}

impl Default for Initialization {
    fn default() -> Self {
        let (ready, _) = tokio::sync::watch::channel(false);
        Self { ready }
    }
}

impl Initialization {
    pub(crate) fn ready(&self) {
        self.ready.send_replace(true);
    }

    async fn wait(&self) -> Result<()> {
        let mut ready = self.ready.subscribe();
        while !*ready.borrow_and_update() {
            ready
                .changed()
                .await
                .context("startup state closed before initialization completed")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Type)]
pub struct Download {
    #[specta(type = f64)]
    pub id: u64,
    pub state: DownloadState,
    pub name: Option<String>,
    #[specta(type = f64)]
    pub completed: u64,
    #[specta(type = f64)]
    pub total: u64,
    pub error: Option<String>,
}

#[derive(Default)]
pub(crate) struct DownloadChannel {
    pub(crate) channel: Mutex<Option<Channel<Download>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum DownloadState {
    Running,
    Finished,
    Failed,
}

#[derive(Clone, Debug, Default, Serialize, Type)]
pub struct ModelResources {
    #[specta(type = f64)]
    pub process_memory: u64,
    #[specta(type = f64)]
    pub system_memory: u64,
    pub process_cpu: f32,
    pub devices: Vec<DeviceResources>,
}

#[derive(Default)]
pub(crate) struct ResourceChannel {
    pub(crate) channel: Mutex<Option<Channel<ModelResources>>>,
}

#[derive(Default)]
pub(crate) struct ProjectChannel {
    pub(crate) channel: Mutex<Option<Channel<Option<ProjectInfo>>>>,
}

#[derive(Clone, Debug, Default, Serialize, Type)]
pub struct DeviceResources {
    pub name: String,
    pub selected: bool,
    #[specta(type = Option<f64>)]
    pub memory_budget: Option<u64>,
    #[specta(type = Option<f64>)]
    pub memory_used: Option<u64>,
    pub utilization: Option<f32>,
}

impl From<koharu_pipeline::ResourceSnapshot> for ModelResources {
    fn from(value: koharu_pipeline::ResourceSnapshot) -> Self {
        Self {
            process_memory: value.process_memory_bytes,
            system_memory: value.system_memory_bytes,
            process_cpu: value.process_cpu_percent,
            devices: value
                .devices
                .into_iter()
                .map(|device| DeviceResources {
                    name: device.name,
                    selected: device.selected,
                    memory_budget: device.memory_budget_bytes,
                    memory_used: device.memory_used_bytes,
                    utilization: device.utilization_percent,
                })
                .collect(),
        }
    }
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn subscribe(
    handle: AppHandle<CefRuntime>,
    on_canvas: Channel<CanvasState>,
    on_job: Channel<Job>,
    on_download: Channel<Download>,
    on_resources: Channel<ModelResources>,
    on_project: Channel<Option<ProjectInfo>>,
) -> std::result::Result<StartupState, Error> {
    handle.state::<Initialization>().wait().await?;

    *handle.state::<CanvasChannel>().channel.lock() = Some(on_canvas);
    *handle.state::<JobChannel>().channel.lock() = Some(on_job);
    *handle.state::<DownloadChannel>().channel.lock() = Some(on_download);
    *handle.state::<ResourceChannel>().channel.lock() = Some(on_resources);
    *handle.state::<ProjectChannel>().channel.lock() = Some(on_project);

    let canvas = handle.state::<Desktop>().canvas_state();
    let preferences = Preferences::load()?;
    Ok(StartupState {
        preferences,
        jobs: handle
            .state::<Processing>()
            .jobs
            .lock()
            .values()
            .cloned()
            .collect(),
        canvas,
    })
}

async fn replace_project(handle: &AppHandle<CefRuntime>, opened: Project) -> Result<()> {
    let snapshot = opened.snapshot();
    let page = opened.active_page();
    let info = opened.info();

    handle.state::<AgentState>().reset().await;
    let processing = handle.state::<Processing>();
    for stop in processing.stops.lock().values() {
        stop.stop();
    }
    processing.stops.lock().clear();
    processing.jobs.lock().clear();

    let previous = {
        let current = handle.state::<CurrentProject>();
        let mut current = current.project.lock().await;
        current.replace(opened)
    };

    let desktop = handle.state::<Desktop>();
    desktop.show_page(&snapshot, page).await?;
    let canvas = desktop.canvas_state();
    drop(previous);
    handle.state::<CanvasChannel>().channel.publish(canvas);
    handle.state::<ProjectChannel>().channel.publish(Some(info));
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_project(
    project: State<'_, CurrentProject>,
) -> std::result::Result<Option<ProjectInfo>, Error> {
    Ok(project.project.lock().await.as_ref().map(Project::info))
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_pages(
    project: State<'_, CurrentProject>,
) -> std::result::Result<Vec<PageSummary>, Error> {
    let snapshot = project
        .project
        .lock()
        .await
        .as_ref()
        .context("no project is open")?
        .snapshot();
    Ok(Project::pages(&snapshot)?)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn get_page(
    project: State<'_, CurrentProject>,
) -> std::result::Result<Option<Page>, Error> {
    let current = {
        let project = project.project.lock().await;
        project
            .as_ref()
            .map(|project| (project.snapshot(), project.active_page()))
    };
    Ok(current
        .and_then(|(snapshot, page)| page.map(|page| (snapshot, page)))
        .map(|(snapshot, page)| Project::page(&snapshot, page))
        .transpose()?)
}

#[tauri::command]
#[specta::specta]
pub(crate) async fn list_projects(
    library: State<'_, ProjectLibrary>,
) -> std::result::Result<Vec<ProjectSummary>, Error> {
    Ok(library.list()?)
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "project_created",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn create_project(
    name: String,
    handle: AppHandle<CefRuntime>,
) -> std::result::Result<(), Error> {
    let library = handle.state::<ProjectLibrary>().inner().clone();
    let opened = library.create(&name).await?;
    replace_project(&handle, opened).await?;
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "project_opened",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn open_project(
    name: String,
    handle: AppHandle<CefRuntime>,
) -> std::result::Result<(), Error> {
    let library = handle.state::<ProjectLibrary>().inner().clone();
    let opened = library.open(&name).await?;
    replace_project(&handle, opened).await?;
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "project_closed",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn close_project(handle: AppHandle<CefRuntime>) -> std::result::Result<(), Error> {
    close_current_project(&handle).await?;
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "project_deleted",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn delete_project(
    name: String,
    handle: AppHandle<CefRuntime>,
) -> std::result::Result<(), Error> {
    let active = handle
        .state::<CurrentProject>()
        .project
        .lock()
        .await
        .as_ref()
        .is_some_and(|project| project.name == name);
    if active {
        close_current_project(&handle).await?;
    }
    let library = handle.state::<ProjectLibrary>().inner().clone();
    tokio::task::spawn_blocking(move || library.delete(&name))
        .await
        .context("project deletion task failed")??;
    Ok(())
}

async fn close_current_project(handle: &AppHandle<CefRuntime>) -> Result<()> {
    handle.state::<AgentState>().reset().await;
    let processing = handle.state::<Processing>();
    for stop in processing.stops.lock().values() {
        stop.stop();
    }
    processing.stops.lock().clear();
    processing.jobs.lock().clear();
    let previous = {
        let current = handle.state::<CurrentProject>();
        let mut current = current.project.lock().await;
        current.take()
    };
    let desktop = handle.state::<Desktop>();
    desktop.clear().await;
    let result = desktop.canvas_state();
    drop(previous);
    handle.state::<CanvasChannel>().channel.publish(result);
    handle.state::<ProjectChannel>().channel.publish(None);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "import",
    skip_all,
    fields(origin = "user", method = ?source),
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn import(
    source: PageImportSource,
    chapter: Option<String>,
    window: WebviewWindow<CefRuntime>,
    desktop: State<'_, Desktop>,
    project: State<'_, CurrentProject>,
    processing: State<'_, Processing>,
    canvas_channel: State<'_, CanvasChannel>,
) -> std::result::Result<(), Error> {
    if !processing.stops.lock().is_empty() {
        return Err(anyhow::anyhow!("pages cannot be imported while processing is running").into());
    }
    let extensions = import::Format::iter()
        .flat_map(|format| format.get_serializations())
        .collect::<Vec<_>>();
    let dialog = rfd::AsyncFileDialog::new()
        .add_filter("Images, archives, and PDF", &extensions)
        .set_parent(&window);
    let files = match source {
        PageImportSource::Files => dialog.pick_files().await.map(|files| {
            files
                .into_iter()
                .map(|file| import::ImportEntry { path: file.path().to_owned(), chapter: None })
                .collect::<Vec<_>>()
        }),
        PageImportSource::Folder => dialog.pick_folder().await.map(|folder| {
            let root = folder.path();
            WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(|entry| match entry {
                    Ok(entry) if entry.file_type().is_file() => Some(entry.into_path()),
                    Ok(_) => None,
                    Err(error) => {
                        tracing::warn!(%error, "could not inspect an import directory entry");
                        None
                    }
                })
                .filter(|path| {
                    path.extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.parse::<import::Format>().is_ok())
                })
                .map(|path| {
                    let chapter = path
                        .strip_prefix(root)
                        .ok()
                        .and_then(|rel| rel.parent())
                        .filter(|parent| *parent != std::path::Path::new(""))
                        .map(|parent| parent.to_string_lossy().into_owned());
                    import::ImportEntry { path, chapter }
                })
                .collect::<Vec<_>>()
        }),
    };
    let Some(files) = files else {
        return Ok(());
    };
    if files.is_empty() {
        return Err(anyhow::anyhow!("no supported images were found in the selection").into());
    }
    let pages = tokio_rayon::spawn(move || import::import_entries(files)).await?;
    let page_count = pages.len();

    let (commit, page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let source = AssetRole::new("source")?;
        let patch = project.snapshot().patch(|edit| {
            for imported in pages {
                let mut draft = PageDraft::new(
                    imported.name,
                    f64::from(imported.width),
                    f64::from(imported.height),
                );
                let assigned_chapter = chapter.as_ref().or(imported.chapter.as_ref());
                if let Some(ch) = assigned_chapter {
                    draft = draft.with_chapter(ch.clone());
                }
                let page = edit.add_page(draft, At::End)?;
                edit.set_asset(
                    page,
                    &source,
                    AssetInput::new(
                        imported.bytes,
                        imported.format.to_mime_type(),
                        AssetMetadata {
                            width: Some(imported.width),
                            height: Some(imported.height),
                            attributes: Default::default(),
                        },
                    ),
                )?;
            }
            Ok(())
        })?;
        let commit = project.session.commit(patch).await?;
        project.record(vec![commit.revision]);
        project.reconcile_page();
        let page = project.active_page();
        (commit, page)
    };
    desktop.synchronize(&commit.snapshot, page, &commit).await?;
    let canvas = desktop.canvas_state();
    canvas_channel.channel.publish(canvas);
    tracing::info!(target: "koharu_metrics", metric = "page_imported", page_count);
    Ok(())
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "page_selected",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn select_page(
    desktop: State<'_, Desktop>,
    page: koharu_scene::EntityId,
    project: State<'_, CurrentProject>,
    canvas_channel: State<'_, CanvasChannel>,
) -> std::result::Result<PageSelection, Error> {
    let (snapshot, project_info, selected_page) = {
        let mut project = project.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        project.select_page(page)?;
        let snapshot = project.snapshot();
        let project_info = project.info();
        let selected_page = Project::page(&snapshot, page)?;
        (snapshot, project_info, selected_page)
    };
    if desktop.show_page(&snapshot, Some(page)).await? {
        let canvas = desktop.canvas_state();
        canvas_channel.channel.publish(canvas);
    }
    Ok(PageSelection {
        project: project_info,
        page: selected_page,
    })
}

#[tracing::instrument(
    target = "koharu_metrics",
    name = "project_imported_from_folder",
    skip_all,
    fields(origin = "user")
)]
#[tauri::command]
#[specta::specta]
pub(crate) async fn import_folder_as_project(
    folder_path: Option<String>,
    window: WebviewWindow<CefRuntime>,
    handle: AppHandle<CefRuntime>,
) -> std::result::Result<(), Error> {
    let folder_path = match folder_path {
        Some(path) => std::path::PathBuf::from(path),
        None => {
            let dialog = rfd::AsyncFileDialog::new().set_parent(&window);
            let Some(folder) = dialog.pick_folder().await else {
                return Ok(());
            };
            folder.path().to_owned()
        }
    };
    let project_name = folder_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("invalid folder name")?
        .to_owned();

    let library = handle.state::<ProjectLibrary>().inner().clone();
    let opened = library.create(&project_name).await?;
    replace_project(&handle, opened).await?;

    let root = folder_path.as_path();
    let mut items = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type().is_file() => Some(entry.into_path()),
            Ok(_) => None,
            Err(error) => {
                tracing::warn!(%error, "could not inspect an import directory entry");
                None
            }
        })
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.parse::<import::Format>().is_ok())
        })
        .map(|path| {
            let chapter = path
                .strip_prefix(root)
                .ok()
                .and_then(|rel| rel.parent())
                .filter(|parent| *parent != std::path::Path::new(""))
                .map(|parent| parent.to_string_lossy().into_owned());
            import::ImportEntry { path, chapter }
        })
        .collect::<Vec<_>>();

    if items.is_empty() {
        return Ok(());
    }

    alphanumeric_sort::sort_slice_by_os_str_key(&mut items, |item| {
        item.path.as_os_str()
    });

    let pages = tokio_rayon::spawn(move || import::import_entries(items)).await?;
    let (commit, page) = {
        let current = handle.state::<CurrentProject>();
        let mut project = current.project.lock().await;
        let project = project.as_mut().context("no project is open")?;
        let source = AssetRole::new("source")?;
        let patch = project.snapshot().patch(|edit| {
            for imported in pages {
                let mut draft = PageDraft::new(
                    imported.name,
                    f64::from(imported.width),
                    f64::from(imported.height),
                );
                if let Some(chapter) = imported.chapter {
                    draft = draft.with_chapter(chapter);
                }
                let page = edit.add_page(draft, At::End)?;
                edit.set_asset(
                    page,
                    &source,
                    AssetInput::new(
                        imported.bytes,
                        imported.format.to_mime_type(),
                        AssetMetadata {
                            width: Some(imported.width),
                            height: Some(imported.height),
                            attributes: Default::default(),
                        },
                    ),
                )?;
            }
            Ok(())
        })?;
        let commit = project.session.commit(patch).await?;
        project.record(vec![commit.revision]);
        project.reconcile_page();
        let page = project.active_page();
        (commit, page)
    };

    let desktop = handle.state::<Desktop>();
    desktop.synchronize(&commit.snapshot, page, &commit).await?;
    let canvas = desktop.canvas_state();
    handle.state::<CanvasChannel>().channel.publish(canvas);
    Ok(())
}
