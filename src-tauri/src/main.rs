#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use novel_studio::{
    library::{ManagedProject, ProjectLibrary},
    project::{EditorDocument, Project, ProjectInfo, Result},
    tasks::{Location, NewTask, Task},
};
use std::{io::Write, path::Path};
use tauri::{AppHandle, Manager};

fn project_library(app: &AppHandle) -> Result<ProjectLibrary> {
    let local = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;
    Ok(ProjectLibrary::new(local.join("projects")))
}

#[tauri::command(async)]
fn open_project(app: AppHandle, root: String) -> Result<ProjectInfo> {
    project_library(&app)?.open_project(Path::new(&root))
}

#[tauri::command(async)]
fn list_managed_projects(app: AppHandle) -> Result<Vec<ManagedProject>> {
    project_library(&app)?.list()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum ImportKind {
    Directory,
    Archive,
    Git,
}

#[tauri::command(async)]
fn import_project(app: AppHandle, source: String, kind: ImportKind) -> Result<ProjectInfo> {
    let library = project_library(&app)?;
    match kind {
        ImportKind::Directory => library.import_directory(Path::new(&source)),
        ImportKind::Archive => library.import_archive(Path::new(&source)),
        ImportKind::Git => library.import_git(Path::new(&source)),
    }
}

#[tauri::command(async)]
fn export_project(app: AppHandle, root: String, destination: String) -> Result<()> {
    let project = Project::open(Path::new(&root))?;
    project_library(&app)?.export(&project, Path::new(&destination))
}

#[tauri::command(async)]
fn project_read(root: String, path: String) -> Result<EditorDocument> {
    Project::open(Path::new(&root))?.read_editor(&path)
}

#[tauri::command(async)]
fn project_image(root: String, path: String) -> Result<String> {
    Project::open(Path::new(&root))?.image_data(&path)
}

#[tauri::command(async)]
fn export_html(destination: String, html: String) -> Result<()> {
    let path = Path::new(&destination);
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("html"))
        || html.len() > 64 * 1024 * 1024
    {
        return Err("请选择 .html 文件名，导出文件最大为 64 MiB。".into());
    }
    let parent = path
        .parent()
        .filter(|parent| parent.is_absolute())
        .ok_or("请选择保存位置。")?;
    let write = || -> std::io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(html.as_bytes())?;
        file.as_file().sync_all()?;
        file.persist_noclobber(path)?;
        Ok(())
    };
    write().map_err(|error| format!("无法导出 HTML：{error}"))
}

#[tauri::command(async)]
fn project_save(
    root: String,
    path: String,
    version: String,
    text: String,
    title: Option<String>,
) -> Result<EditorDocument> {
    Project::open(Path::new(&root))?.save_editor(&path, &version, &text, title.as_deref())
}

#[tauri::command(async)]
fn project_tasks(root: String) -> Result<Vec<Task>> {
    Project::open(Path::new(&root))?.tasks()
}

#[tauri::command(async)]
fn create_task(root: String, input: NewTask) -> Result<Task> {
    Project::open(Path::new(&root))?.create_task(input)
}

#[tauri::command(async)]
fn locate_task(root: String, task_id: String) -> Result<Option<Location>> {
    let project = Project::open(Path::new(&root))?;
    let task = project
        .tasks()?
        .into_iter()
        .find(|task| task.id == task_id)
        .ok_or("找不到任务。")?;
    Ok(project.task_location(&task))
}

#[tauri::command(async)]
fn review_proposal(root: String, task_id: String, proposal_id: String, apply: bool) -> Result<()> {
    Project::open(Path::new(&root))?.review(&task_id, &proposal_id, apply)
}

#[tauri::command]
fn mcp_program() -> Result<Option<String>> {
    let program = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name(format!("novel-studio-mcp{}", std::env::consts::EXE_SUFFIX));
    Ok(program
        .is_file()
        .then(|| program.to_string_lossy().into_owned()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            open_project,
            list_managed_projects,
            import_project,
            export_project,
            project_read,
            project_image,
            export_html,
            project_save,
            project_tasks,
            create_task,
            locate_task,
            review_proposal,
            mcp_program,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Novel Studio");
}
