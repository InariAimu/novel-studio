use crate::project::{valid_document_path, write_atomic, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::HashSet, fs, path::Path};
use uuid::Uuid;

const FILE_NAME: &str = "project.json";

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    General,
    Manual,
    Notes,
    Novel,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Markdown {
    Commonmark,
    #[default]
    Gfm,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    schema_version: u32,
    pub name: String,
    pub preset: Preset,
    #[serde(default)]
    pub markdown: Markdown,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub documents: Vec<ManifestDocument>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_commit: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ManifestDocument {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unnumbered: Option<bool>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl ManifestDocument {
    pub fn new(path: String, kind: String, unnumbered: bool) -> Self {
        Self {
            path,
            id: Some(Uuid::new_v4().to_string()),
            kind: Some(kind),
            unnumbered: unnumbered.then_some(true),
            extra: Map::new(),
        }
    }
}

impl Manifest {
    pub fn new(name: String, preset: Preset) -> Self {
        Self {
            schema_version: 1,
            name,
            preset,
            markdown: Markdown::default(),
            documents: Vec::new(),
            source_commit: None,
            extra: Map::new(),
        }
    }

    pub fn read(state_dir: &Path) -> Result<Option<Self>> {
        let path = state_dir.join(FILE_NAME);
        match path.symlink_metadata() {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Ok(metadata) if metadata.is_file() && metadata.len() <= 1024 * 1024 => {}
            _ => return Err("项目清单必须是 1 MiB 以内的普通文件。".into()),
        }
        let text =
            fs::read_to_string(path).map_err(|error| format!("无法读取项目清单：{error}"))?;
        let manifest: Self = serde_json::from_str(text.trim_start_matches('\u{feff}'))
            .map_err(|error| format!("无法解析项目清单：{error}"))?;
        if manifest.schema_version != 1 {
            return Err(format!("不支持项目清单版本 {}。", manifest.schema_version));
        }
        if manifest.name.trim().is_empty() {
            return Err("项目清单缺少项目名称。".into());
        }
        let mut paths = HashSet::new();
        let mut ids = HashSet::new();
        for document in &manifest.documents {
            if !valid_document_path(&document.path) || !paths.insert(document.path.to_lowercase()) {
                return Err(format!("项目清单包含无效或重复的文档：{}", document.path));
            }
            if document
                .id
                .as_deref()
                .is_some_and(|id| Uuid::parse_str(id).map_or(true, |uuid| !ids.insert(uuid)))
            {
                return Err(format!(
                    "项目清单包含无效或重复的文档 ID：{}",
                    document.path
                ));
            }
            if document.kind.as_deref().is_some_and(|kind| {
                !matches!(
                    kind,
                    "chapter"
                        | "summary"
                        | "worldbuilding"
                        | "writing"
                        | "illustration"
                        | "material"
                        | "generated"
                )
            }) {
                return Err(format!("项目清单包含未知的文档角色：{}", document.path));
            }
        }
        if manifest.source_commit.as_deref().is_some_and(|id| {
            !matches!(id.len(), 40 | 64) || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err("项目清单中的来源提交标识无效。".into());
        }
        Ok(Some(manifest))
    }

    pub fn write(&self, state_dir: &Path) -> Result<()> {
        fs::create_dir_all(state_dir).map_err(|error| format!("无法创建项目数据目录：{error}"))?;
        let text = serde_json::to_string_pretty(self).map_err(|error| error.to_string())?;
        if text.len() >= 1024 * 1024 {
            return Err("项目清单超过 1 MiB。".into());
        }
        write_atomic(&state_dir.join(FILE_NAME), &format!("{text}\n"))
    }
}
