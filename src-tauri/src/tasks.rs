use crate::project::{write_atomic, Document, EditorDocument, Project, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs};
use uuid::Uuid;

#[derive(Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Workspace {
    tasks: Vec<Task>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Task {
    pub id: String,
    pub path: String,
    pub instruction: String,
    pub anchor: Anchor,
    pub proposals: Vec<Proposal>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Anchor {
    pub quote: String,
    prefix: String,
    suffix: String,
    start: usize,
    version: String,
}

#[derive(Serialize)]
pub struct Location {
    pub path: String,
    pub version: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    path: String,
    version: String,
    start: usize,
    end: usize,
    instruction: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Proposal {
    pub id: String,
    pub summary: String,
    pub changes: Vec<Change>,
    pub status: ProposalStatus,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Applied,
    Rejected,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub path: String,
    pub version: String,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProposal {
    task_id: String,
    summary: String,
    changes: Vec<Change>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingApply {
    task_id: String,
    proposal_id: String,
    files: Vec<PendingFile>,
}

#[derive(Deserialize, Serialize)]
struct PendingFile {
    path: String,
    before: String,
    after: String,
}

#[derive(Serialize)]
pub struct Reference {
    pub path: String,
    pub version: String,
    pub text: String,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct TaskContext {
    pub id: String,
    pub instruction: String,
    pub quote: String,
    pub location: Option<Location>,
    pub nearby: Option<String>,
    pub references: Vec<Reference>,
}

#[derive(Serialize)]
pub struct Match {
    pub path: String,
    pub line: usize,
    pub text: String,
}

impl Project {
    fn load_workspace(&self) -> Result<Workspace> {
        let path = self.state_dir()?.join("tasks.json");
        match path.symlink_metadata() {
            Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                return Err("项目任务文件无效。".into());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Workspace::default()),
            Err(e) => return Err(format!("无法访问任务文件：{e}")),
            Ok(_) => {}
        }
        let data = fs::read(path).map_err(|e| format!("无法读取任务：{e}"))?;
        serde_json::from_slice(&data).map_err(|e| format!("无法解析任务文件：{e}"))
    }

    fn store_workspace(&self, workspace: &Workspace) -> Result<()> {
        let dir = self.state_dir()?;
        match fs::create_dir(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(format!("无法创建项目数据目录：{e}")),
        }
        let json = serde_json::to_string_pretty(workspace).map_err(|e| e.to_string())?;
        write_atomic(&dir.join("tasks.json"), &format!("{json}\n"))
    }

    pub fn save(&self, path: &str, version: &str, text: &str) -> Result<Document> {
        self.save_with(path, version, |_| Ok(text.to_owned()))
    }

    pub fn save_editor(
        &self,
        path: &str,
        version: &str,
        text: &str,
        title: Option<&str>,
    ) -> Result<EditorDocument> {
        let document = self.save_with(path, version, |document| {
            self.markdown_parts(document)
                .save(&document.text, text, title)
        })?;
        Ok(self.editor_document(document))
    }

    fn save_with(
        &self,
        path: &str,
        version: &str,
        edit: impl FnOnce(&Document) -> Result<String>,
    ) -> Result<Document> {
        let _lock = self.lock()?;
        let mut workspace = self.load_workspace()?;
        self.finish_pending(&mut workspace)?;
        self.require_editable(path)?;
        let document = self.read(path)?;
        document.require_version(version)?;
        let text = edit(&document)?;
        if text != document.text {
            let raw = document.encode(&text)?;
            write_atomic(&self.document_path(path)?, &raw)?;
        }
        self.read(path)
    }

    pub fn recover(&self) -> Result<()> {
        let _lock = self.lock()?;
        let mut workspace = self.load_workspace()?;
        self.finish_pending(&mut workspace)
    }

    pub fn tasks(&self) -> Result<Vec<Task>> {
        let _lock = self.lock()?;
        Ok(self.load_workspace()?.tasks)
    }

    pub(crate) fn validate_tasks(&self) -> Result<()> {
        self.load_workspace().map(|_| ())
    }

    pub fn create_task(&self, input: NewTask) -> Result<Task> {
        let _lock = self.lock()?;
        let mut workspace = self.load_workspace()?;
        self.require_editable(&input.path)?;
        let document = self.read(&input.path)?;
        document.require_version(&input.version)?;
        let instruction = input.instruction.trim();
        if instruction.is_empty() || instruction.len() > 16_384 {
            return Err("请填写任务说明（最多 16 KiB）。".into());
        }
        let quote = document
            .text
            .get(input.start..input.end)
            .filter(|text| !text.trim().is_empty() && text.len() <= 32_768)
            .ok_or("请选择一段不超过 32 KiB 的原文。")?;
        let prefix = tail(&document.text[..input.start], 100).to_owned();
        let suffix = head(&document.text[input.end..], 100).to_owned();
        let task = Task {
            id: Uuid::new_v4().to_string(),
            path: input.path,
            instruction: instruction.to_owned(),
            anchor: Anchor {
                quote: quote.to_owned(),
                prefix,
                suffix,
                start: input.start,
                version: document.version,
            },
            proposals: Vec::new(),
        };
        workspace.tasks.push(task.clone());
        self.store_workspace(&workspace)?;
        Ok(task)
    }

    pub fn task_location(&self, task: &Task) -> Option<Location> {
        if task.anchor.quote.is_empty() {
            return None;
        }
        if let Ok(document) = self.read(&task.path) {
            if document.version == task.anchor.version {
                return task
                    .anchor
                    .start
                    .checked_add(task.anchor.quote.len())
                    .filter(|end| {
                        document.text.get(task.anchor.start..*end) == Some(&task.anchor.quote)
                    })
                    .map(|end| Location {
                        path: document.path,
                        version: document.version,
                        start: task.anchor.start,
                        end,
                    });
            }
        }
        let kind = self.document_kind(&task.path)?;
        let mut best = None;
        let mut score = 0;
        let mut ambiguous = false;
        for entry in self
            .documents()
            .into_iter()
            .filter(|entry| entry.kind == kind)
        {
            let Ok(document) = self.read(&entry.path) else {
                continue;
            };
            for (start, quote) in document.text.match_indices(&task.anchor.quote) {
                let end = start + quote.len();
                let prefix_matches = !task.anchor.prefix.is_empty()
                    && document.text[..start].ends_with(&task.anchor.prefix);
                let suffix_matches = !task.anchor.suffix.is_empty()
                    && document.text[end..].starts_with(&task.anchor.suffix);
                let candidate = 1
                    + 4 * usize::from(prefix_matches)
                    + 4 * usize::from(suffix_matches)
                    + usize::from(document.path == task.path);
                if candidate > score {
                    score = candidate;
                    ambiguous = false;
                    best = Some(Location {
                        path: entry.path.clone(),
                        version: document.version.clone(),
                        start,
                        end,
                    });
                } else if candidate == score {
                    ambiguous = true;
                }
            }
        }
        if ambiguous {
            None
        } else {
            best
        }
    }

    pub fn context(&self, task_id: &str) -> Result<TaskContext> {
        let _lock = self.lock()?;
        let workspace = self.load_workspace()?;
        let task = workspace
            .tasks
            .iter()
            .find(|task| task.id == task_id)
            .ok_or("找不到任务。")?;
        let location = self.task_location(task);
        let path = location
            .as_ref()
            .map_or(task.path.as_str(), |location| &location.path);
        let nearby = location.as_ref().and_then(|location| {
            let doc = self.read(&location.path).ok()?;
            doc.require_version(&location.version).ok()?;
            Some(format!(
                "{}{}{}",
                tail(&doc.text[..location.start], 1000),
                &doc.text[location.start..location.end],
                head(&doc.text[location.end..], 1000)
            ))
        });
        let mut paths = ["AGENTS.md", "instruction.md"].map(String::from).to_vec();
        if self.document_kind(path) == Some("chapter") {
            let chapters: Vec<_> = self
                .documents()
                .into_iter()
                .filter(|entry| entry.kind == "chapter")
                .collect();
            if let Some(index) = chapters.iter().position(|entry| entry.path == path) {
                for chapter in chapters[index.saturating_sub(1)..=index].iter().rev() {
                    if let Some(name) = chapter.path.strip_prefix("manuscript/") {
                        paths.push(format!("manuscript/summaries/{name}"));
                    }
                }
            }
        }
        paths.extend(["worldbuilding/canon.md", "worldbuilding/timeline.md"].map(String::from));
        let references = paths
            .into_iter()
            .filter_map(|path| {
                let doc = self.read(&path).ok()?;
                let text = head(&doc.text, 4000);
                Some(Reference {
                    path,
                    version: doc.version,
                    truncated: text.len() < doc.text.len(),
                    text: text.to_owned(),
                })
            })
            .collect();
        Ok(TaskContext {
            id: task.id.clone(),
            instruction: task.instruction.clone(),
            quote: task.anchor.quote.clone(),
            location,
            nearby,
            references,
        })
    }

    pub fn search(&self, query: &str) -> Result<Vec<Match>> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() || needle.len() > 256 {
            return Err("请输入 1 至 256 字节的搜索词。".into());
        }
        let mut matches = Vec::new();
        for entry in self
            .documents()
            .into_iter()
            .filter(|entry| entry.kind != "generated")
        {
            let Ok(doc) = self.read(&entry.path) else {
                continue;
            };
            for (index, line) in doc.text.lines().enumerate() {
                if line.to_lowercase().contains(&needle) {
                    matches.push(Match {
                        path: entry.path.clone(),
                        line: index + 1,
                        text: head(line, 500).into(),
                    });
                    if matches.len() == 30 {
                        return Ok(matches);
                    }
                }
            }
        }
        Ok(matches)
    }

    pub fn submit(&self, mut input: NewProposal) -> Result<Proposal> {
        for change in &mut input.changes {
            if change.new_text.contains('\r') {
                change.new_text = change.new_text.replace("\r\n", "\n").replace('\r', "\n");
            }
        }
        let _lock = self.lock()?;
        let mut workspace = self.load_workspace()?;
        let task = workspace
            .tasks
            .iter_mut()
            .find(|task| task.id == input.task_id)
            .ok_or("找不到任务。")?;
        let summary = input.summary.trim();
        if let Some(existing) = task.proposals.iter().find(|proposal| {
            proposal.status != ProposalStatus::Rejected
                && proposal.summary == summary
                && proposal.changes == input.changes
        }) {
            return Ok(existing.clone());
        }
        if completed(task) {
            return Err("任务已完成。".into());
        }
        if summary.is_empty() || summary.len() > 32_768 || input.changes.len() > 20 {
            return Err("请填写建议说明（最多 32 KiB），单次最多修改 20 个文件。".into());
        }
        self.prepare_changes(&input.changes)?;
        let proposal = Proposal {
            id: Uuid::new_v4().to_string(),
            summary: summary.to_owned(),
            changes: input.changes,
            status: ProposalStatus::Pending,
        };
        task.proposals.push(proposal.clone());
        self.store_workspace(&workspace)?;
        Ok(proposal)
    }

    pub fn review(&self, task_id: &str, proposal_id: &str, apply: bool) -> Result<()> {
        let _lock = self.lock()?;
        let mut workspace = self.load_workspace()?;
        self.finish_pending(&mut workspace)?;
        let task = workspace
            .tasks
            .iter_mut()
            .find(|task| task.id == task_id)
            .ok_or("找不到任务。")?;
        let already_completed = completed(task);
        let proposal = task
            .proposals
            .iter_mut()
            .find(|proposal| proposal.id == proposal_id)
            .ok_or("找不到建议。")?;
        if proposal.status
            == (if apply {
                ProposalStatus::Applied
            } else {
                ProposalStatus::Rejected
            })
        {
            return Ok(());
        }
        if proposal.status != ProposalStatus::Pending {
            return Err("这份建议已经处理。".into());
        }
        if !apply {
            proposal.status = ProposalStatus::Rejected;
            return self.store_workspace(&workspace);
        }
        if already_completed {
            return Err("这项任务已应用另一份建议。".into());
        }
        let files = self.prepare_changes(&proposal.changes)?;
        let pending = PendingApply {
            task_id: task_id.into(),
            proposal_id: proposal_id.into(),
            files,
        };
        let serialized = serde_json::to_string(&pending).map_err(|e| e.to_string())?;
        write_atomic(&self.runtime_file("apply.json")?, &serialized)?;
        self.finish_pending(&mut workspace)
    }

    fn prepare_changes(&self, changes: &[Change]) -> Result<Vec<PendingFile>> {
        let mut seen = HashSet::new();
        let mut files = Vec::with_capacity(changes.len());
        let mut bytes = 0usize;
        for change in changes {
            self.require_editable(&change.path)?;
            let path = self.document_path(&change.path)?;
            if !seen.insert(path) {
                return Err("每个文件请提供一处连续替换。".into());
            }
            bytes = bytes
                .saturating_add(change.old_text.len())
                .saturating_add(change.new_text.len());
            if bytes > 2 * 1024 * 1024 {
                return Err("单份建议的原文和替换内容合计不能超过 2 MiB。".into());
            }
            let document = self.read(&change.path)?;
            document.require_version(&change.version)?;
            if change.old_text.is_empty()
                || change.old_text == change.new_text
                || document.text.match_indices(&change.old_text).count() != 1
            {
                return Err(format!(
                    "替换原文需要在文件里唯一匹配，并且确实发生改动：{}",
                    change.path
                ));
            }
            let text = document
                .text
                .replacen(&change.old_text, &change.new_text, 1);
            let after = document.encode(&text)?;
            files.push(PendingFile {
                path: change.path.clone(),
                before: document.raw,
                after,
            });
        }
        Ok(files)
    }

    fn finish_pending(&self, workspace: &mut Workspace) -> Result<()> {
        let journal = self.runtime_file("apply.json")?;
        let data = match fs::read(&journal) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("无法读取应用记录：{e}")),
        };
        let pending: PendingApply =
            serde_json::from_slice(&data).map_err(|e| format!("无法解析应用记录：{e}"))?;
        let proposal = workspace
            .tasks
            .iter_mut()
            .find(|task| task.id == pending.task_id)
            .and_then(|task| {
                task.proposals
                    .iter_mut()
                    .find(|proposal| proposal.id == pending.proposal_id)
            });
        let remove_journal =
            || fs::remove_file(&journal).map_err(|e| format!("无法清理应用记录：{e}"));
        if proposal
            .as_ref()
            .is_some_and(|proposal| proposal.status == ProposalStatus::Applied)
        {
            return remove_journal();
        }
        let state = |file: &PendingFile| self.read(&file.path).ok().map(|document| document.raw);
        let conflict = pending.files.iter().find(|file| {
            let raw = state(file);
            raw.as_ref() != Some(&file.before) && raw.as_ref() != Some(&file.after)
        });
        let valid_proposal = proposal
            .as_ref()
            .is_some_and(|proposal| proposal.status == ProposalStatus::Pending);
        if conflict.is_some() || !valid_proposal {
            // Revert only writes still matching this transaction, preserving external edits.
            for file in &pending.files {
                if state(file).as_ref() == Some(&file.after) {
                    write_atomic(&self.document_path(&file.path)?, &file.before)?;
                }
            }
            remove_journal()?;
            return Err(conflict.map_or_else(
                || "待恢复的建议已被外部修改。".into(),
                |file| format!("外部编辑与应用建议冲突：{}。请重新审阅。", file.path),
            ));
        }
        for file in &pending.files {
            let current = self.read(&file.path)?;
            if current.raw == file.before {
                write_atomic(&self.document_path(&file.path)?, &file.after)?;
            } else if current.raw != file.after {
                return Err(format!(
                    "应用期间文件已修改：{}。请重新打开项目恢复。",
                    file.path
                ));
            }
        }
        proposal.ok_or("找不到待恢复的建议。")?.status = ProposalStatus::Applied;
        self.store_workspace(workspace)?;
        remove_journal()
    }
}

pub fn completed(task: &Task) -> bool {
    task.proposals
        .iter()
        .any(|proposal| proposal.status == ProposalStatus::Applied)
}

pub fn head(text: &str, chars: usize) -> &str {
    &text[..text
        .char_indices()
        .nth(chars)
        .map_or(text.len(), |(index, _)| index)]
}

fn tail(text: &str, chars: usize) -> &str {
    &text[text
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map_or(0, |(index, _)| index)..]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const CHAPTER: &str = "manuscript/01-opening.md";
    const SUMMARY: &str = "manuscript/summaries/01-opening.md";
    const QUOTE: &str = "她推开门。🌒";

    fn fixture() -> (tempfile::TempDir, Project) {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir_all(temp.path().join("manuscript/summaries")).unwrap();
        fs::write(
            temp.path().join(CHAPTER),
            format!("\u{feff}# Chapter 1: Opening\r\n\r\n{QUOTE}\r\n\r\nThe wind fell.\r\n"),
        )
        .unwrap();
        fs::write(temp.path().join(SUMMARY), "**Summary:** She entered.\n").unwrap();
        let project = Project::open(temp.path()).unwrap();
        (temp, project)
    }

    fn assign(project: &Project) -> Task {
        let document = project.read(CHAPTER).unwrap();
        let start = document.text.find(QUOTE).unwrap();
        project
            .create_task(NewTask {
                path: CHAPTER.into(),
                version: document.version,
                start,
                end: start + QUOTE.len(),
                instruction: "核对该段与摘要".into(),
            })
            .unwrap()
    }

    fn change(project: &Project, path: &str, before: &str, after: &str) -> Change {
        Change {
            path: path.into(),
            version: project.read(path).unwrap().version,
            old_text: before.into(),
            new_text: after.into(),
        }
    }

    fn suggest(project: &Project, task: &Task) -> Proposal {
        project
            .submit(NewProposal {
                task_id: task.id.clone(),
                summary: "对齐正文和摘要".into(),
                changes: vec![
                    change(project, CHAPTER, QUOTE, "她关上门。🌒"),
                    change(project, SUMMARY, "She entered.", "She closed the door."),
                ],
            })
            .unwrap()
    }

    #[test]
    fn applies_reviewed_files_and_keeps_encoding() {
        let (dir, project) = fixture();
        let task = assign(&project);
        let proposal = suggest(&project, &task);
        let other_process = Project::open(dir.path()).unwrap();
        assert!(other_process.read(CHAPTER).unwrap().text.contains(QUOTE));
        other_process.review(&task.id, &proposal.id, true).unwrap();
        other_process.review(&task.id, &proposal.id, true).unwrap();

        let raw = fs::read_to_string(dir.path().join(CHAPTER)).unwrap();
        assert!(raw.starts_with("\u{feff}# Chapter 1: Opening\r\n\r\n她关上门。🌒\r\n"));
        assert!(project
            .read(SUMMARY)
            .unwrap()
            .text
            .contains("She closed the door."));
        assert!(completed(&project.tasks().unwrap()[0]));
        assert!(project.read("../outside.md").is_err());
    }

    #[test]
    fn stale_proposal_does_not_change_other_files() {
        let (dir, project) = fixture();
        let task = assign(&project);
        let proposal = suggest(&project, &task);
        let external = "**Summary:** Edited elsewhere.\n";
        fs::write(dir.path().join(SUMMARY), external).unwrap();

        assert!(project.review(&task.id, &proposal.id, true).is_err());
        assert!(project.read(CHAPTER).unwrap().text.contains(QUOTE));
        assert_eq!(project.read(SUMMARY).unwrap().text, external);
        assert!(project.tasks().unwrap()[0].proposals[0].status == ProposalStatus::Pending);
    }

    #[test]
    fn relocates_a_renumbered_chapter_after_reopening() {
        let (dir, project) = fixture();
        let task = assign(&project);
        let new_path = Path::new("manuscript/02-opening.md");
        fs::rename(dir.path().join(CHAPTER), dir.path().join(new_path)).unwrap();
        let new_raw = project
            .read(new_path.to_str().unwrap())
            .unwrap()
            .raw
            .replace("Chapter 1", "Chapter 2");
        fs::write(dir.path().join(new_path), new_raw).unwrap();

        let project = Project::open(dir.path()).unwrap();
        let context = project.context(&task.id).unwrap();
        assert_eq!(context.location.unwrap().path, "manuscript/02-opening.md");
        assert_eq!(context.quote, QUOTE);
    }

    #[test]
    fn resumes_interrupted_multifile_review() {
        let (dir, project) = fixture();
        let task = assign(&project);
        let proposal = suggest(&project, &task);
        let files = project.prepare_changes(&proposal.changes).unwrap();
        let pending = PendingApply {
            task_id: task.id,
            proposal_id: proposal.id,
            files,
        };
        write_atomic(
            &project.runtime_file("apply.json").unwrap(),
            &serde_json::to_string(&pending).unwrap(),
        )
        .unwrap();
        write_atomic(&dir.path().join(CHAPTER), &pending.files[0].after).unwrap();

        let project = Project::open(dir.path()).unwrap();
        project.recover().unwrap();
        assert!(project.read(CHAPTER).unwrap().text.contains("她关上门。🌒"));
        assert!(project
            .read(SUMMARY)
            .unwrap()
            .text
            .contains("She closed the door."));
        assert!(completed(&project.tasks().unwrap()[0]));
        assert!(!project.runtime_file("apply.json").unwrap().exists());
    }
}
