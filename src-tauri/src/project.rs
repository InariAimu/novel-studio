use crate::{
    manifest::{Manifest, ManifestDocument, Markdown, Preset},
    markdown::Markdown as MarkdownParts,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;
use walkdir::WalkDir;

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct Project {
    root: PathBuf,
    pub(crate) manifest: Option<Manifest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub root: String,
    pub name: String,
    pub git: bool,
    pub managed: bool,
    pub preset: Preset,
    pub markdown: Markdown,
    pub source_commit: Option<String>,
    pub baseline_commit: Option<String>,
    pub documents: Vec<DocumentEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentEntry {
    pub id: Option<String>,
    pub path: String,
    pub title: String,
    pub kind: String,
    pub chapter_number: Option<usize>,
}

#[derive(Serialize)]
pub struct Document {
    pub path: String,
    pub text: String,
    pub version: String,
    #[serde(skip)]
    pub raw: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDocument {
    pub path: String,
    pub text: String,
    pub version: String,
    pub title: Option<String>,
    pub body_start: usize,
    pub needs_title_migration: bool,
}

impl Project {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .map_err(|e| format!("无法打开项目目录：{e}"))?;
        if !root.is_dir() {
            return Err("请选择项目目录。".into());
        }
        let mut project = Self {
            root,
            manifest: None,
        };
        project.manifest = Manifest::read(&project.state_dir()?)?;
        Ok(project)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub fn info(&self) -> ProjectInfo {
        let root = display_path(&self.root);
        let documents = self.documents();
        let folder = self
            .root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| root.clone());
        let name = self.manifest.as_ref().map_or_else(
            || {
                documents
                    .iter()
                    .find(|entry| entry.path.eq_ignore_ascii_case("README.md"))
                    .map_or(folder, |entry| entry.title.clone())
            },
            |manifest| manifest.name.clone(),
        );
        ProjectInfo {
            root,
            name,
            git: self.root.join(".git").exists(),
            managed: false,
            preset: self.preset(),
            markdown: self
                .manifest
                .as_ref()
                .map_or(Markdown::Gfm, |manifest| manifest.markdown),
            source_commit: self
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.source_commit.clone()),
            baseline_commit: None,
            documents,
        }
    }

    pub(crate) fn preset(&self) -> Preset {
        self.manifest.as_ref().map_or_else(
            || {
                if self.root.join("manuscript").is_dir() || self.root.join("worldbuilding").is_dir()
                {
                    Preset::Novel
                } else {
                    Preset::General
                }
            },
            |manifest| manifest.preset,
        )
    }

    pub fn documents(&self) -> Vec<DocumentEntry> {
        let mut entries = WalkDir::new(&self.root)
            .max_depth(24)
            .into_iter()
            .filter_entry(|entry| {
                entry.depth() == 0
                    || (visible(&entry.file_name().to_string_lossy())
                        && !entry.file_type().is_symlink()
                        && entry
                            .path()
                            .canonicalize()
                            .is_ok_and(|path| path == entry.path()))
            })
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if !entry.file_type().is_file() {
                    return None;
                }
                let path = entry.path().strip_prefix(&self.root).ok()?;
                let relative = path.to_str()?.replace('\\', "/");
                let kind = self.document_kind(&relative)?.to_owned();
                let document = self.read(&relative).ok()?;
                let fallback = file_title(&relative);
                let parts = MarkdownParts::read(&document.text, kind == "chapter", fallback);
                let title = parts.title.unwrap_or_else(|| {
                    parts
                        .body
                        .lines()
                        .find_map(|line| line.strip_prefix("# ").map(str::trim))
                        .filter(|title| !title.is_empty())
                        .unwrap_or(fallback)
                        .to_owned()
                });
                Some(DocumentEntry {
                    id: self
                        .manifest_entry(&relative)
                        .and_then(|entry| entry.id.clone()),
                    path: relative,
                    title,
                    kind,
                    chapter_number: None,
                })
            })
            .collect::<Vec<_>>();
        let order_of = |path: &str| {
            self.manifest.as_ref().and_then(|manifest| {
                manifest
                    .documents
                    .iter()
                    .position(|entry| entry.path.eq_ignore_ascii_case(path))
            })
        };
        entries.sort_by_cached_key(|entry| {
            (
                order_of(&entry.path).unwrap_or(usize::MAX),
                order(entry),
                entry.path.clone(),
            )
        });
        let mut chapter_number = 0;
        for entry in &mut entries {
            if entry.kind == "chapter" && !self.is_unnumbered(&entry.path) {
                chapter_number += 1;
                entry.chapter_number = Some(chapter_number);
            }
        }
        entries
    }

    pub(crate) fn register_documents(&self, info: &mut ProjectInfo) -> Result<()> {
        let mut manifest = self
            .manifest
            .clone()
            .unwrap_or_else(|| Manifest::new(info.name.clone(), info.preset));
        let mut order: HashMap<_, _> = manifest
            .documents
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.path.to_lowercase(), index))
            .collect();
        let mut changed = false;
        for entry in &mut info.documents {
            let unnumbered = entry.kind == "chapter" && entry.chapter_number.is_none();
            if let Some(&index) = order.get(&entry.path.to_lowercase()) {
                let saved = &mut manifest.documents[index];
                if saved.id.is_none() {
                    saved.id = Some(Uuid::new_v4().to_string());
                    changed = true;
                }
                if unnumbered && saved.unnumbered.is_none() {
                    saved.unnumbered = Some(true);
                    changed = true;
                }
                entry.id = saved.id.clone();
            } else {
                let saved =
                    ManifestDocument::new(entry.path.clone(), entry.kind.clone(), unnumbered);
                entry.id = saved.id.clone();
                order.insert(entry.path.to_lowercase(), manifest.documents.len());
                manifest.documents.push(saved);
                changed = true;
            }
        }
        if changed {
            manifest.write(&self.state_dir()?)?;
        }
        Ok(())
    }

    fn manifest_entry(&self, path: &str) -> Option<&ManifestDocument> {
        self.manifest
            .as_ref()?
            .documents
            .iter()
            .find(|entry| entry.path.eq_ignore_ascii_case(path))
    }

    fn is_unnumbered(&self, path: &str) -> bool {
        self.manifest_entry(path)
            .and_then(|entry| entry.unnumbered)
            .unwrap_or_else(|| {
                path.rsplit('/')
                    .next()
                    .is_some_and(|name| name.starts_with("00-"))
            })
    }

    pub(crate) fn document_kind(&self, path: &str) -> Option<&str> {
        if !valid_document_path(path) {
            return None;
        }
        self.manifest_entry(path)
            .and_then(|entry| entry.kind.as_deref())
            .or_else(|| {
                Some(
                    if self.manifest.is_none() || matches!(self.preset(), Preset::Novel) {
                        novel_document_kind(path)
                    } else {
                        "material"
                    },
                )
            })
    }

    pub(crate) fn require_editable(&self, path: &str) -> Result<()> {
        match self.document_kind(path) {
            Some("generated") => Err(format!("请从正文重新生成此文件：{path}")),
            Some(_) => Ok(()),
            None => Err(format!("无效的文档路径：{path}")),
        }
    }

    pub fn document_path(&self, relative: &str) -> Result<PathBuf> {
        if !valid_document_path(relative) {
            return Err(format!("项目内没有可访问的 Markdown 文档：{relative}"));
        }
        self.file_path(relative)
    }

    fn file_path(&self, relative: &str) -> Result<PathBuf> {
        let mut path = self.root.clone();
        for part in relative.split('/') {
            path.push(part);
            let metadata = fs::symlink_metadata(&path).map_err(|e| format!("{relative}：{e}"))?;
            if metadata.file_type().is_symlink() {
                return Err(format!("无法访问符号链接：{relative}"));
            }
        }
        let resolved = path
            .canonicalize()
            .map_err(|e| format!("{relative}：{e}"))?;
        if resolved != path || !resolved.is_file() {
            return Err(format!("无法访问项目文档：{relative}"));
        }
        Ok(resolved)
    }

    pub fn image_data(&self, relative: &str) -> Result<String> {
        const MAX_BYTES: u64 = 12 * 1024 * 1024;
        if !valid_relative_path(relative) {
            return Err(format!("无效的项目图片路径：{relative}"));
        }
        let path = self.file_path(relative)?;
        let file = File::open(&path).map_err(|error| format!("{relative}：{error}"))?;
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("{relative}：{error}"))?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(format!("图片超过 12 MiB：{relative}"));
        }
        let mime = match bytes.as_slice() {
            [137, b'P', b'N', b'G', 13, 10, 26, 10, ..] => "image/png",
            [255, 216, 255, ..] => "image/jpeg",
            [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => "image/gif",
            [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
            _ => return Err(format!("图片格式不受支持：{relative}")),
        };
        Ok(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
    }

    pub fn read(&self, relative: &str) -> Result<Document> {
        let path = self.document_path(relative)?;
        let metadata = path.metadata().map_err(|e| format!("{relative}：{e}"))?;
        if metadata.len() > MAX_DOCUMENT_BYTES as u64 {
            return Err(format!("文档超过 8 MiB：{relative}"));
        }
        let raw = fs::read_to_string(path).map_err(|e| format!("{relative}：{e}"))?;
        if raw.len() > MAX_DOCUMENT_BYTES {
            return Err(format!("文档超过 8 MiB：{relative}"));
        }
        let text = raw
            .strip_prefix('\u{feff}')
            .unwrap_or(&raw)
            .replace("\r\n", "\n")
            .replace('\r', "\n");
        let version = hash(&raw);
        Ok(Document {
            path: relative.into(),
            text,
            version,
            raw,
        })
    }

    pub fn read_editor(&self, path: &str) -> Result<EditorDocument> {
        Ok(self.editor_document(self.read(path)?))
    }

    pub(crate) fn editor_document(&self, document: Document) -> EditorDocument {
        let parts = self.markdown_parts(&document);
        EditorDocument {
            path: document.path.clone(),
            version: document.version.clone(),
            body_start: document.text.len() - parts.body.len(),
            text: parts.body.to_owned(),
            title: parts.title,
            needs_title_migration: parts.needs_title_migration,
        }
    }

    pub(crate) fn markdown_parts<'a>(&self, document: &'a Document) -> MarkdownParts<'a> {
        MarkdownParts::read(
            &document.text,
            self.document_kind(&document.path) == Some("chapter"),
            file_title(&document.path),
        )
    }

    pub(crate) fn state_dir(&self) -> Result<PathBuf> {
        let path = self.root.join(".novel-studio");
        match path.symlink_metadata() {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => {
                Err("项目数据目录无效。".into())
            }
            Ok(_) if path.canonicalize().ok().as_ref() != Some(&path) => {
                Err("项目数据目录无效。".into())
            }
            Ok(_) => Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(path),
            Err(e) => Err(format!("无法访问项目数据目录：{e}")),
        }
    }

    pub(crate) fn runtime_file(&self, extension: &str) -> Result<PathBuf> {
        let key = hash(&self.root.to_string_lossy());
        let local = std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let dir = local.join("novel-studio").join("project-runtime");
        fs::create_dir_all(&dir).map_err(|e| format!("无法创建本地项目运行目录：{e}"))?;
        Ok(dir.join(format!("{key}.{extension}")))
    }

    pub(crate) fn lock(&self) -> Result<File> {
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.runtime_file("lock")?)
            .map_err(|e| format!("无法锁定项目：{e}"))?;
        lock.lock().map_err(|e| format!("无法锁定项目：{e}"))?;
        Ok(lock)
    }
}

fn file_title(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(path)
}

pub(crate) fn display_path(path: &Path) -> String {
    let path = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(path) = path.strip_prefix(r"\\?\") {
            return path.to_owned();
        }
    }
    path.into_owned()
}

impl Document {
    pub(crate) fn encode(&self, text: &str) -> Result<String> {
        let breaks = self.raw.matches("\r\n").count();
        let content = if breaks > 0 && breaks == self.raw.matches('\n').count() {
            text.replace('\n', "\r\n")
        } else {
            text.to_owned()
        };
        let raw = if self.raw.starts_with('\u{feff}') {
            format!("\u{feff}{content}")
        } else {
            content
        };
        if raw.len() > MAX_DOCUMENT_BYTES {
            return Err("保存后的文档超过 8 MiB。".into());
        }
        Ok(raw)
    }

    pub(crate) fn require_version(&self, version: &str) -> Result<()> {
        if self.version != version {
            return Err(format!("文件已在别处更新，重新读取后再处理：{}", self.path));
        }
        Ok(())
    }
}

pub(crate) fn valid_document_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".md") && valid_relative_path(path)
}

fn valid_relative_path(path: &str) -> bool {
    path.split('/').all(visible)
        && Path::new(path)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn novel_document_kind(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if matches!(lower.as_str(), "novel.md" | "word-counts.md") {
        "generated"
    } else if lower.starts_with("manuscript/summaries/") {
        "summary"
    } else if lower.starts_with("manuscript/") && lower.matches('/').count() == 1 {
        "chapter"
    } else if lower.starts_with("worldbuilding/") {
        "worldbuilding"
    } else if lower.starts_with("prompts/writing/") {
        "writing"
    } else if lower.starts_with("prompts/illustration/") {
        "illustration"
    } else {
        "material"
    }
}

fn visible(part: &str) -> bool {
    !part.is_empty()
        && !part.starts_with('.')
        && !part.ends_with(['.', ' '])
        && !part.contains(['\\', ':', '<', '>', '"', '|', '?', '*'])
        && !part.chars().any(char::is_control)
        && !matches!(part, "node_modules" | "target" | "dist" | "__pycache__")
}

fn order(entry: &DocumentEntry) -> (u8, u32) {
    let rank = match entry.kind.as_str() {
        "chapter" => 0,
        "summary" => 1,
        "worldbuilding" => 2,
        "writing" => 3,
        "illustration" => 4,
        "material" => 5,
        _ => 6,
    };
    let number = entry.path.rsplit('/').next().and_then(|name| {
        name.split_once('-')
            .and_then(|(prefix, _)| prefix.parse().ok())
    });
    (rank, number.unwrap_or(u32::MAX))
}

pub(crate) fn hash(contents: &str) -> String {
    format!("{:x}", Sha256::digest(contents.as_bytes()))
}

pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().ok_or("缺少文件所在目录。")?;
    let replace = || -> std::io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(contents.as_bytes())?;
        if let Ok(metadata) = path.metadata() {
            file.as_file().set_permissions(metadata.permissions())?;
        }
        file.as_file().sync_all()?;
        file.persist(path)?;
        Ok(())
    };
    replace().map_err(|e| format!("无法保存 {}：{e}", path.display()))
}
