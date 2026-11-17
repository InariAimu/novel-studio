use crate::project::{display_path, write_atomic, Project, ProjectInfo, Result};
use crate::{git_import, manifest::Manifest};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use tempfile::{NamedTempFile, TempDir};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const WORKSPACE: &str = "workspace";
const RECORD: &str = "project.json";
const ARCHIVE_PREFIX: &[u8] = b"Novel Studio project 1\n";

pub struct ProjectLibrary(PathBuf);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedProject {
    pub root: String,
    pub name: String,
    pub imported_at: u64,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    name: String,
    imported_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baseline_commit: Option<String>,
}

impl ProjectLibrary {
    pub fn new(root: PathBuf) -> Self {
        Self(root)
    }

    pub fn list(&self) -> Result<Vec<ManagedProject>> {
        let dirs = match fs::read_dir(&self.0) {
            Ok(dirs) => dirs,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!("无法读取项目库：{error}")),
        };
        let mut projects = Vec::new();
        for entry in dirs {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
                || Uuid::parse_str(&entry.file_name().to_string_lossy()).is_err()
            {
                continue;
            }
            let directory = entry.path();
            let root = directory.join(WORKSPACE);
            if let Ok(record) = read_record(&directory) {
                if root.is_dir() {
                    projects.push(ManagedProject {
                        root: display_path(&root),
                        name: Project::open(&root)
                            .ok()
                            .and_then(|project| project.manifest)
                            .map_or(record.name, |manifest| manifest.name),
                        imported_at: record.imported_at,
                    });
                }
            }
        }
        projects.sort_by(|a, b| {
            b.imported_at
                .cmp(&a.imported_at)
                .then_with(|| a.root.cmp(&b.root))
        });
        Ok(projects)
    }

    pub fn describe(&self, project: &Project) -> ProjectInfo {
        let mut info = project.info();
        if project
            .root()
            .file_name()
            .is_some_and(|name| name == WORKSPACE)
        {
            if let (Some(parent), Ok(library)) = (project.root().parent(), self.0.canonicalize()) {
                if parent.parent() == Some(library.as_path())
                    && parent
                        .file_name()
                        .is_some_and(|id| Uuid::parse_str(&id.to_string_lossy()).is_ok())
                {
                    if let Ok(record) = read_record(parent) {
                        info.managed = true;
                        if project.manifest.is_none() {
                            info.name = record.name;
                        }
                        info.baseline_commit = record.baseline_commit;
                    }
                }
            }
        }
        info
    }

    pub fn open_project(&self, root: &Path) -> Result<ProjectInfo> {
        let project = Project::open(root)?;
        project.recover()?;
        let _lock = project.lock()?;
        let project = Project::open(project.root())?;
        let mut info = self.describe(&project);
        project.register_documents(&mut info)?;
        Ok(info)
    }

    pub fn import_directory(&self, source: &Path) -> Result<ProjectInfo> {
        let project = Project::open(source)?;
        let library = self.directory()?;
        if library.starts_with(project.root()) {
            return Err("请选择项目目录；所选目录包含 Studio 项目库。".into());
        }
        let (_lock, entries) = snapshot(&project)?;
        let name = self.describe(&project).name;
        let stage = new_stage(&library)?;
        let workspace = stage.path().join(WORKSPACE);
        for entry in &entries {
            let output = workspace.join(&entry.relative);
            if entry.directory {
                create_directory(&output)?;
            } else {
                let input = project.root().join(&entry.relative);
                let mut file = File::open(&input).map_err(|error| file_error(&input, error))?;
                create_file(&output, &mut file, entry.size)?;
            }
        }
        confirm_snapshot(&project, &entries)?;
        self.adopt(stage, name, false)
    }

    pub fn import_git(&self, source: &Path) -> Result<ProjectInfo> {
        let source = source
            .canonicalize()
            .map_err(|error| file_error(source, error))?;
        let id = git_import::head(&source)?;
        let stage = new_stage(&self.directory()?)?;
        let workspace = stage.path().join(WORKSPACE);
        git_import::copy_tree(&source, &id, &workspace)?;
        let project = Project::open(&workspace)?;
        let info = project.info();
        let mut manifest = match project.manifest.clone() {
            Some(manifest) => manifest,
            None => {
                let name = if info
                    .documents
                    .iter()
                    .any(|entry| entry.path.eq_ignore_ascii_case("README.md"))
                {
                    info.name
                } else {
                    source
                        .file_name()
                        .map_or(info.name, |name| name.to_string_lossy().into_owned())
                };
                Manifest::new(project_name(name), info.preset)
            }
        };
        manifest.source_commit = Some(id);
        manifest.write(&project.state_dir()?)?;
        self.adopt(stage, manifest.name, true)
    }

    pub fn import_archive(&self, source: &Path) -> Result<ProjectInfo> {
        let file = File::open(source).map_err(|error| file_error(source, error))?;
        let mut zip = ZipArchive::new(BufReader::new(file))
            .map_err(|error| format!("无法读取项目包：{error}"))?;
        let title = zip
            .comment()
            .strip_prefix(ARCHIVE_PREFIX)
            .ok_or("请选择由 Novel Studio 导出的项目 ZIP。")?;
        let name: String = serde_json::from_slice(title).map_err(|_| "项目包中的名称无效。")?;

        let stage = new_stage(&self.directory()?)?;
        let workspace = stage.path().join(WORKSPACE);
        for index in 0..zip.len() {
            let mut entry = zip
                .by_index(index)
                .map_err(|error| format!("无法读取项目包条目：{error}"))?;
            let name =
                std::str::from_utf8(entry.name_raw()).map_err(|_| "项目包包含无效文件名。")?;
            let directory = entry.is_dir();
            let relative = checked_path(if directory {
                name.strip_suffix('/').unwrap_or(name)
            } else {
                name
            })?;
            if entry.is_symlink()
                || entry.unix_mode().is_some_and(|mode| {
                    let kind = mode & 0o170000;
                    kind != 0 && kind != if directory { 0o040000 } else { 0o100000 }
                })
            {
                return Err(format!("项目包包含非普通文件：{name}"));
            }
            let output = workspace.join(relative);
            if directory {
                create_directory(&output)?;
            } else {
                let size = entry.size();
                let parent = output.parent().ok_or("项目包路径无效。")?;
                create_directory(parent)?;
                create_file(&output, &mut entry, size)?;
            }
        }
        self.adopt(stage, name, false)
    }

    pub fn export(&self, project: &Project, destination: &Path) -> Result<()> {
        let filename = destination.file_name().ok_or("请选择 ZIP 文件名。")?;
        let parent = destination
            .parent()
            .ok_or("请选择项目包目标目录。")?
            .canonicalize()
            .map_err(|error| file_error(destination, error))?;
        if parent.starts_with(project.root()) {
            return Err("请将 ZIP 导出到当前项目目录之外。".into());
        }
        let destination = parent.join(filename);
        if destination.symlink_metadata().is_ok() {
            return Err("同名项目包已存在，请选择另一个文件名。".into());
        }
        let (_lock, entries) = snapshot(project)?;
        let mut output =
            NamedTempFile::new_in(&parent).map_err(|error| file_error(&parent, error))?;
        let mut zip = ZipWriter::new(output.as_file_mut());
        let name = serde_json::to_string(&project_name(self.describe(project).name))
            .map_err(|error| error.to_string())?;
        let comment = format!("{}{name}", String::from_utf8_lossy(ARCHIVE_PREFIX));
        zip.set_comment(comment)
            .map_err(|error| error.to_string())?;
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for entry in &entries {
            if entry.directory {
                zip.add_directory(format!("{}/", entry.name), options)
                    .map_err(|error| file_error(&entry.relative, error))?;
            } else {
                zip.start_file(&entry.name, options.large_file(entry.size >= 0xffff_0000))
                    .map_err(|error| file_error(&entry.relative, error))?;
                let input = project.root().join(&entry.relative);
                let file = File::open(&input).map_err(|error| file_error(&input, error))?;
                copy_exact(file, &mut zip, entry.size)
                    .map_err(|error| file_error(&input, error))?;
            }
        }
        zip.finish()
            .map_err(|error| format!("无法完成项目包：{error}"))?;
        confirm_snapshot(project, &entries)?;
        output
            .as_file()
            .sync_all()
            .map_err(|error| file_error(&destination, error))?;
        output
            .persist_noclobber(&destination)
            .map_err(|error| file_error(&destination, error.error))?;
        Ok(())
    }

    fn directory(&self) -> Result<PathBuf> {
        fs::create_dir_all(&self.0).map_err(|error| file_error(&self.0, error))?;
        self.0
            .canonicalize()
            .map_err(|error| file_error(&self.0, error))
    }

    fn adopt(&self, stage: TempDir, name: String, git_baseline: bool) -> Result<ProjectInfo> {
        let staged = Project::open(&stage.path().join(WORKSPACE))?;
        staged.validate_tasks()?;
        let name = project_name(name);
        let mut info = staged.info();
        info.name = name.clone();
        staged.register_documents(&mut info)?;
        let record = Record {
            name,
            imported_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            baseline_commit: git_baseline
                .then(|| git_import::baseline(staged.root()))
                .transpose()?,
        };
        let json = serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?;
        write_atomic(&stage.path().join(RECORD), &json)?;
        let root = stage
            .path()
            .parent()
            .ok_or("项目库路径无效。")?
            .join(Uuid::new_v4().to_string());
        fs::rename(stage.path(), &root).map_err(|error| file_error(&root, error))?;
        Ok(self.describe(&Project::open(&root.join(WORKSPACE))?))
    }
}

fn project_name(name: String) -> String {
    let name: String = name.trim().chars().take(160).collect();
    if name.is_empty() {
        "小说项目".into()
    } else {
        name
    }
}

fn read_record(dir: &Path) -> Result<Record> {
    let file = dir.join(RECORD);
    let data = fs::read(&file).map_err(|error| file_error(&file, error))?;
    serde_json::from_slice(&data).map_err(|error| file_error(&file, error))
}

fn new_stage(library: &Path) -> Result<TempDir> {
    let stage = tempfile::Builder::new()
        .prefix(".import-")
        .tempdir_in(library)
        .map_err(|error| file_error(library, error))?;
    create_directory(&stage.path().join(WORKSPACE))?;
    Ok(stage)
}

#[derive(PartialEq)]
struct TreeEntry {
    relative: PathBuf,
    name: String,
    directory: bool,
    size: u64,
    modified: Option<SystemTime>,
}

fn snapshot(project: &Project) -> Result<(File, Vec<TreeEntry>)> {
    let lock = project.lock()?;
    match project.runtime_file("apply.json")?.symlink_metadata() {
        Ok(_) => {
            return Err("项目有尚未完成的建议应用。请先在 Studio 打开项目以恢复，再重试。".into())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("无法检查项目的建议应用状态：{error}")),
    }
    Ok((lock, scan_tree(project.root())?))
}

fn confirm_snapshot(project: &Project, expected: &[TreeEntry]) -> Result<()> {
    if scan_tree(project.root())? != expected {
        return Err("项目文件在传输期间发生了变化，请重试。".into());
    }
    Ok(())
}

fn scan_tree(root: &Path) -> Result<Vec<TreeEntry>> {
    WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !entry.file_name().eq_ignore_ascii_case(".git"))
        .skip(1)
        .map(|entry| {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            if (!entry.file_type().is_file() && !entry.file_type().is_dir())
                || path
                    .canonicalize()
                    .map_err(|error| file_error(path, error))?
                    != path
            {
                return Err(format!("项目包含链接或非普通文件：{}", display_path(path)));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|error| file_error(path, error))?;
            let parts = relative
                .iter()
                .map(|part| part.to_str().ok_or("项目包含无法编码的文件名。"));
            let name = parts.collect::<std::result::Result<Vec<_>, _>>()?.join("/");
            let relative = checked_path(&name)?;
            let directory = entry.file_type().is_dir();
            let metadata = entry.metadata().map_err(|error| file_error(path, error))?;
            Ok(TreeEntry {
                relative,
                name,
                directory,
                size: if directory { 0 } else { metadata.len() },
                modified: if directory {
                    None
                } else {
                    metadata.modified().ok()
                },
            })
        })
        .collect()
}

pub(crate) fn checked_path(name: &str) -> Result<PathBuf> {
    let mut path = PathBuf::new();
    for part in name.split('/') {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end()
            .to_ascii_uppercase();
        let device = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        });
        if part.is_empty()
            || part.eq_ignore_ascii_case(".git")
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| {
                c.is_control() || matches!(c, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
            })
            || device
        {
            return Err(format!("项目文件名无法安全导入：{name}"));
        }
        path.push(part);
    }
    Ok(path)
}

pub(crate) fn create_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|error| file_error(path, error))?;
    if path
        .canonicalize()
        .map_err(|error| file_error(path, error))?
        != path
    {
        return Err(format!("项目包目录的名称发生冲突：{}", display_path(path)));
    }
    Ok(())
}

pub(crate) fn create_file(path: &Path, reader: &mut impl Read, size: u64) -> Result<()> {
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| file_error(path, error))?;
    copy_exact(reader, &mut file, size).map_err(|error| file_error(path, error))
}

fn copy_exact(input: impl Read, output: &mut impl io::Write, expected: u64) -> io::Result<()> {
    let count = io::copy(&mut input.take(expected.saturating_add(1)), output)?;
    if count != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "文件大小与记录不符",
        ));
    }
    Ok(())
}

fn file_error(path: &Path, error: impl std::fmt::Display) -> String {
    format!("{}：{error}", display_path(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Cursor, Write};

    #[test]
    fn registers_legacy_documents_and_keeps_task_anchors_after_heading_migration() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("novel");
        fs::create_dir_all(root.join("manuscript")).unwrap();
        let path = "manuscript/10-bells.md";
        let legacy = "# Chapter 10: 雾港\r\n\r\n潮水涨过港湾。\r\n";
        for (file, text) in [
            ("manuscript/00-front-matter.md", "# 卷首\n"),
            (path, legacy),
            ("manuscript/20-lights.md", "# 下一章\n\n海面起风。\n"),
            ("README.md", "# 雾港纪事\n"),
            ("novel.md", "# 汇编\n"),
        ] {
            fs::write(root.join(file), text).unwrap();
        }
        let library = ProjectLibrary::new(temp.path().join("library"));
        let first = library.open_project(&root).unwrap();
        assert_eq!(fs::read_to_string(root.join(path)).unwrap(), legacy);
        let ordered = first
            .documents
            .iter()
            .filter(|entry| entry.kind == "chapter")
            .map(|entry| (entry.title.as_str(), entry.chapter_number))
            .collect::<Vec<_>>();
        assert_eq!(
            ordered,
            [("卷首", None), ("雾港", Some(1)), ("下一章", Some(2))]
        );
        assert!(first.documents.iter().all(|entry| entry.id.is_some()));

        let reopened = library.open_project(&root).unwrap();
        assert!(first
            .documents
            .iter()
            .zip(reopened.documents)
            .all(|(old, new)| old.path == new.path && old.id == new.id));
        let project = Project::open(&root).unwrap();
        let legacy_editor = project.read_editor(path).unwrap();
        assert_eq!(legacy_editor.title.as_deref(), Some("雾港"));
        assert_eq!(legacy_editor.text, "潮水涨过港湾。\n");
        assert!(legacy_editor.needs_title_migration);
        let quote = "潮水涨过港湾";
        let task = project
            .create_task(
                serde_json::from_value(json!({
                    "path": path, "version": legacy_editor.version, "instruction": "核对原文",
                    "start": legacy_editor.body_start, "end": legacy_editor.body_start + quote.len()
                }))
                .unwrap(),
            )
            .unwrap();
        let saved = project
            .save_editor(
                path,
                &legacy_editor.version,
                &legacy_editor.text,
                Some("港湾来潮"),
            )
            .unwrap();
        assert_eq!(saved.title.as_deref(), Some("港湾来潮"));
        assert_eq!(saved.text, legacy_editor.text);
        assert!(!saved.needs_title_migration);
        assert_eq!(
            fs::read_to_string(root.join(path)).unwrap(),
            "---\r\ntitle: \"港湾来潮\"\r\n---\r\n\r\n潮水涨过港湾。\r\n"
        );
        let location = project.task_location(&task).unwrap();
        assert_eq!(location.start, saved.body_start);
        assert_eq!(location.end, saved.body_start + quote.len());
        let after = library.open_project(&root).unwrap();
        let by_path = |entries: &[crate::project::DocumentEntry], path: &str| {
            entries
                .iter()
                .find(|entry| entry.path == path)
                .unwrap()
                .id
                .clone()
        };
        assert_eq!(
            by_path(&first.documents, path),
            by_path(&after.documents, path)
        );
    }

    #[test]
    fn git_import_preserves_committed_material_and_records_a_clean_baseline() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("原始仓库");
        fs::create_dir_all(source.join("assets")).unwrap();
        fs::create_dir_all(source.join(".novel-studio")).unwrap();
        let original = "# 产品说明书\r\n\r\n![图](assets/图.png)\r\n";
        let image = include_bytes!("../../examples/product-manual/assets/novel-studio.png");
        fs::write(source.join("guide.md"), original).unwrap();
        fs::write(source.join("assets/图.png"), image).unwrap();
        fs::write(
            source.join(".gitattributes"),
            "assets/ export-ignore\n*.md text\n",
        )
        .unwrap();
        fs::write(
            source.join(".novel-studio/project.json"),
            r#"{
            "schemaVersion": 1, "name": "产品说明书", "preset": "manual",
                "markdown": "commonmark", "publisher": "样例团队",
                "documents": [{ "path": "guide.md", "revision": 2 }]
        }"#,
        )
        .unwrap();
        let source_commit = git_import::baseline(&source).unwrap();
        fs::write(source.join("guide.md"), "本地未提交的编辑").unwrap();
        fs::write(source.join("scratch.md"), "本地未跟踪资料").unwrap();

        let library = ProjectLibrary::new(temp.path().join("library"));
        let imported = library.import_git(&source).unwrap();
        assert!(imported.managed);
        assert_eq!(imported.name, "产品说明书");
        assert_eq!(
            imported.source_commit.as_deref(),
            Some(source_commit.as_str())
        );
        let root = Path::new(&imported.root);
        let baseline = imported.baseline_commit.as_deref().unwrap();
        assert_ne!(baseline, source_commit);
        assert_eq!(git_import::head(root).unwrap(), baseline);
        assert!(!root.join("scratch.md").exists());
        assert_eq!(fs::read_to_string(root.join("guide.md")).unwrap(), original);
        assert_eq!(fs::read(root.join("assets/图.png")).unwrap(), image);
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(".novel-studio/project.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["publisher"], "样例团队");
        assert_eq!(manifest["documents"][0]["revision"], 2);
        assert_eq!(
            fs::read_to_string(source.join("guide.md")).unwrap(),
            "本地未提交的编辑"
        );
        assert_eq!(git_import::head(&source).unwrap(), source_commit);

        let project = Project::open(root).unwrap();
        assert!(project
            .image_data("assets/图.png")
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert!(project.image_data("../原始仓库/assets/图.png").is_err());
        let zip = temp.path().join("manual.zip");
        library.export(&project, &zip).unwrap();
        let restored = library.import_archive(&zip).unwrap();
        assert_eq!(restored.source_commit, imported.source_commit);
        assert!(restored.baseline_commit.is_none());
    }

    #[test]
    fn round_trip_preserves_all_material_and_review_history() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("原稿");
        let chapter = "manuscript/01-opening.md";
        let setting = "worldbuilding/canon.md";
        let original = "# 第一章\n\n她走进城门。\n\n守卫等着她。\n";
        for dir in [
            "manuscript",
            "worldbuilding",
            "assets/empty",
            ".git",
            ".notes",
        ] {
            fs::create_dir_all(source.join(dir)).unwrap();
        }
        fs::write(source.join(chapter), original).unwrap();
        fs::write(source.join(setting), "# 设定\n\n这座城只有一道门。").unwrap();
        fs::write(source.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        fs::write(source.join(".gitignore"), "*.cache\n").unwrap();
        fs::write(source.join(".notes/想法.md"), "下一章写城墙。").unwrap();
        fs::write(source.join("assets/插图.bin"), [0, 255, 8, 10, 13]).unwrap();

        let project = Project::open(&source).unwrap();
        let version = project.read(chapter).unwrap().version;
        let quote = "她走进城门。";
        let start = original.find(quote).unwrap();
        let task = project
            .create_task(
                serde_json::from_value(json!({
                    "path": chapter, "version": version, "start": start, "end": start + quote.len(),
                    "instruction": "核对守卫的动作。"
                }))
                .unwrap(),
            )
            .unwrap();
        let submit = |sentence: &str| {
            project
                .submit(
                    serde_json::from_value(json!({
                        "taskId": task.id, "summary": sentence,
                        "changes": [{
                            "path": chapter, "version": version,
                            "oldText": "守卫等着她。", "newText": sentence
                        }]
                    }))
                    .unwrap(),
                )
                .unwrap()
        };
        let rejected = submit("守卫追赶她。");
        let applied = submit("守卫向她点头。");
        project.review(&task.id, &rejected.id, false).unwrap();
        project.review(&task.id, &applied.id, true).unwrap();

        let library_root = temp.path().join("library");
        let library = ProjectLibrary::new(library_root.clone());
        let imported = library.import_directory(&source).unwrap();
        assert!(imported.managed && !imported.git);
        assert_eq!(imported.name, "原稿");
        assert_eq!(library.list().unwrap().len(), 1);
        let managed = Project::open(Path::new(&imported.root)).unwrap();
        assert_eq!(managed.context(&task.id).unwrap().quote, quote);
        assert!(managed.context(&task.id).unwrap().location.is_some());
        let document = managed.read(chapter).unwrap();
        managed
            .save(
                chapter,
                &document.version,
                &(document.text + "\n城门已上锁。\n"),
            )
            .unwrap();
        assert!(!project.read(chapter).unwrap().text.contains("城门已上锁"));

        let archive = temp.path().join("renamed.zip");
        library.export(&managed, &archive).unwrap();
        assert!(library.export(&managed, &archive).is_err());
        let restored = library.import_archive(&archive).unwrap();
        assert!(restored.managed && !restored.git);
        assert_eq!(restored.name, imported.name);
        assert_ne!(restored.root, imported.root);
        assert_eq!(ProjectLibrary::new(library_root).list().unwrap().len(), 2);

        let restored = Project::open(Path::new(&restored.root)).unwrap();
        let tree = scan_tree(managed.root()).unwrap();
        let restored_tree = scan_tree(restored.root()).unwrap();
        assert_eq!(
            tree.iter().map(|entry| &entry.name).collect::<Vec<_>>(),
            restored_tree
                .iter()
                .map(|entry| &entry.name)
                .collect::<Vec<_>>()
        );
        for entry in tree.into_iter().filter(|entry| !entry.directory) {
            assert_eq!(
                fs::read(managed.root().join(&entry.relative)).unwrap(),
                fs::read(restored.root().join(&entry.relative)).unwrap(),
                "{}",
                entry.name
            );
        }
        assert!(restored.root().join("assets/empty").is_dir());
        assert!(!restored.root().join(".git").exists());
        assert_eq!(restored.context(&task.id).unwrap().quote, quote);
        assert!(restored.context(&task.id).unwrap().location.is_some());
        let tasks = serde_json::to_value(restored.tasks().unwrap()).unwrap();
        assert_eq!(tasks[0]["proposals"][0]["status"], "rejected");
        assert_eq!(tasks[0]["proposals"][1]["status"], "applied");
    }

    #[test]
    fn invalid_archive_cannot_publish_partial_project() {
        let temp = tempfile::tempdir().unwrap();
        let library_root = temp.path().join("library");
        let library = ProjectLibrary::new(library_root.clone());
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.set_comment("Novel Studio project 1\n\"Broken\"")
            .unwrap();
        zip.start_file("manuscript/01.md", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"# First chapter\n").unwrap();
        zip.start_file("ok/escaped.md", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"outside").unwrap();
        let mut bytes = zip.finish().unwrap().into_inner();
        // Replace the same-length names in both headers to exercise extraction checks.
        let name = b"ok/escaped.md";
        for index in 0..=bytes.len() - name.len() {
            if &bytes[index..index + name.len()] == name {
                bytes[index..index + name.len()].copy_from_slice(b"../escaped.md");
            }
        }
        let archive = temp.path().join("broken.zip");
        fs::write(&archive, bytes).unwrap();
        assert!(library.import_archive(&archive).is_err());
        assert!(library.list().unwrap().is_empty());
        assert_eq!(fs::read_dir(library_root).unwrap().count(), 0);
    }
}
