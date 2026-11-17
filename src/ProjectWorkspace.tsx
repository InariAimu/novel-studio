import { ask, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { ClipboardPlus, Code2, Download, Eye, FileText, Folder, RefreshCw, Save } from 'lucide-react';
import { useEffect, useRef, useState, type FormEvent } from 'react';
import { LocalImage, ProjectMarkdown, projectPath } from './ProjectMarkdown';
import { projectApi, type DocumentEntry, type DocumentKind, type Location, type Project, type ProjectDocument, type Proposal, type Task } from './project';
import TaskReview from './TaskReview';

export type ProjectPage = 'overview' | 'chapters' | 'writing' | 'illustration' | 'tasks';
type Filter = DocumentKind | 'all';
interface Buffer { document: ProjectDocument; draft: string; title: string | null }
interface Selection { path: string; start: number; end: number }
interface Assignment extends Selection { quote: string }

const groups: [Filter, string][] = [
  ['all', '全部文档'], ['chapter', '章节'], ['summary', '摘要'], ['worldbuilding', '世界观'],
  ['writing', '写作提示词'], ['illustration', '插图提示词'], ['material', '其他物料'], ['generated', '生成产物'],
];
const openBuffer = (document: ProjectDocument): Buffer => ({ document, draft: document.text, title: document.title });
const isDirty = (buffer: Buffer) => buffer.draft !== buffer.document.text || buffer.title !== buffer.document.title;
const encoder = new TextEncoder();
const decoder = new TextDecoder();

interface Props {
  project: Project;
  page: ProjectPage | null;
  locked: boolean;
  onNavigate: (page: ProjectPage) => void;
  onUpdate: (project: Project) => void;
  onDirty: (dirty: boolean) => void;
}

export default function ProjectWorkspace({ project, page, locked, onNavigate, onUpdate, onDirty }: Props) {
  const [filter, setFilter] = useState<Filter>(() => project.preset === 'novel'
    && project.documents.some(entry => entry.kind === 'chapter') ? 'chapter' : 'all');
  const [activePath, setActivePath] = useState('');
  const [buffers, setBuffers] = useState<Record<string, Buffer>>({});
  const [preview, setPreview] = useState(false);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [assignment, setAssignment] = useState<Assignment | null>(null);
  const [instruction, setInstruction] = useState('');
  const [tasks, setTasks] = useState<Task[]>([]);
  const [selectedTask, setSelectedTask] = useState<string | null>(null);
  const [location, setLocation] = useState<Location | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const [notice, setNotice] = useState('');
  const editor = useRef<HTMLTextAreaElement>(null);
  const titleInput = useRef<HTMLInputElement>(null);
  const dialog = useRef<HTMLDialogElement>(null);
  const editing = page === 'chapters' || page === 'writing' || page === 'illustration';
  const kind = page === 'writing' || page === 'illustration' ? page : filter;
  const entries = project.documents.filter(entry => kind === 'all' || entry.kind === kind);
  const entry = entries.find(entry => entry.path === activePath) ?? entries[0];
  const path = editing ? entry?.path : undefined;
  const buffer = path ? buffers[path] : undefined;
  const canSave = buffer && (isDirty(buffer) || buffer.document.needsTitleMigration);
  const selectedText = buffer && selection && selection.path === path
    ? buffer.draft.slice(selection.start, selection.end)
    : '';
  const dirty = Object.values(buffers).some(isDirty);

  useEffect(() => onDirty(dirty), [dirty, onDirty]);

  useEffect(() => {
    if (!path || buffer) return;
    let current = true;
    void projectApi.read(project.root, path).then(document => {
      if (current) setBuffers(old => ({ ...old, [path]: openBuffer(document) }));
    }).catch(reason => { if (current) setError(String(reason)); });
    return () => { current = false; };
  }, [path, project.root, buffer]);

  useEffect(() => {
    if (page !== 'tasks') return;
    let current = true;
    const refresh = () => {
      void projectApi.tasks(project.root).then(items => {
        if (current) setTasks(items);
      }).catch(reason => { if (current) setError(String(reason)); });
    };
    refresh();
    const timer = window.setInterval(refresh, 2200);
    return () => { current = false; window.clearInterval(timer); };
  }, [page, project.root]);

  useEffect(() => {
    if (assignment && !dialog.current?.open) dialog.current?.showModal();
  }, [assignment]);

  useEffect(() => {
    const input = editor.current;
    if (!location || !input || !buffer || location.path !== path || location.version !== buffer.document.version) return;
    setLocation(null);
    const start = location.start - buffer.document.bodyStart;
    const end = location.end - buffer.document.bodyStart;
    if (start < 0) {
      titleInput.current?.focus();
      setNotice('原文位于文档标题或 frontmatter。');
      return;
    }
    const bytes = encoder.encode(buffer.draft);
    input.focus();
    input.setSelectionRange(
      decoder.decode(bytes.slice(0, start)).length,
      decoder.decode(bytes.slice(0, end)).length,
    );
    setSelection({ path: location.path, start: input.selectionStart, end: input.selectionEnd });
  }, [location, path, buffer, preview]);

  function select(path: string) {
    setActivePath(path);
    setSelection(null);
    setError('');
    setNotice('');
  }

  function openEntry(entry: DocumentEntry) {
    setFilter(entry.kind);
    select(entry.path);
    onNavigate('chapters');
  }

  async function perform(run: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError('');
    setNotice('');
    try {
      await run();
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function save(current: Buffer) {
    const document = await projectApi.save(project.root, current.document, current.draft, current.title);
    setBuffers(old => ({ ...old, [document.path]: openBuffer(document) }));
    return document;
  }

  function saveBuffer(current: Buffer) {
    void perform(async () => { await save(current); setNotice('已保存'); });
  }

  async function reload() {
    if (!path || (buffer && isDirty(buffer) && !await ask('放弃此文档尚未保存的改动，重新读取磁盘？', { title: '重新读取' }))) return;
    await perform(async () => {
      const document = await projectApi.read(project.root, path);
      setBuffers(old => ({ ...old, [path]: openBuffer(document) }));
      onUpdate(await projectApi.open(project.root));
      setSelection(null);
      setNotice('已重新读取');
    });
  }

  function beginAssignment() {
    if (!selection || !selectedText.trim()) return;
    setAssignment({ ...selection, quote: selectedText });
    setInstruction('');
    setError('');
  }

  function assign(event: FormEvent) {
    event.preventDefault();
    if (!assignment || !instruction.trim()) return;
    const current = buffers[assignment.path];
    if (!current) return;
    void perform(async () => {
      const document = isDirty(current) ? await save(current) : current.document;
      if (document.text.slice(assignment.start, assignment.end) !== assignment.quote) {
        throw new Error('保存后选区已变化，请重新选择原文。');
      }
      const task = await projectApi.createTask(project.root, {
        path: document.path, version: document.version, instruction,
        start: document.bodyStart + encoder.encode(document.text.slice(0, assignment.start)).length,
        end: document.bodyStart + encoder.encode(document.text.slice(0, assignment.end)).length,
      });
      setTasks(items => [...items, task]);
      setSelectedTask(task.id);
      setAssignment(null);
      setNotice('已交办，可在「任务与审阅」中查看');
    });
  }

  function refreshTasks() {
    void perform(async () => setTasks(await projectApi.tasks(project.root)));
  }

  function locate(task: Task) {
    void perform(async () => {
      const target = await projectApi.locate(project.root, task.id);
      if (!target) throw new Error('原文已改写或存在多个匹配，需人工核对标注。');
      const targetEntry = project.documents.find(item => item.path === target.path);
      if (!targetEntry) {
        const updated = await projectApi.open(project.root);
        onUpdate(updated);
      }
      if (buffers[target.path] && isDirty(buffers[target.path])) throw new Error('请先保存该文档的草稿，再定位原文。');
      const document = await projectApi.read(project.root, target.path);
      if (document.version !== target.version) throw new Error('定位期间文档已变化，请重试。');
      setBuffers(old => ({ ...old, [target.path]: openBuffer(document) }));
      setFilter(targetEntry?.kind ?? 'all');
      select(target.path);
      setPreview(false);
      setLocation(target);
      onNavigate('chapters');
    });
  }

  function review(task: Task, proposal: Proposal, apply: boolean) {
    void perform(async () => {
      if (apply && proposal.changes.some(({ path }) => buffers[path] && isDirty(buffers[path]))) {
        throw new Error('建议涉及的文档有未保存的改动，请先保存或重新读取。');
      }
      await projectApi.review(project.root, task.id, proposal.id, apply);
      if (apply && proposal.changes.length) {
        setBuffers(old => {
          const next = { ...old };
          for (const change of proposal.changes) delete next[change.path];
          return next;
        });
      }
      setTasks(await projectApi.tasks(project.root));
      if (apply) onUpdate(await projectApi.open(project.root));
      if (!apply) setNotice('建议已退回');
      else setNotice(proposal.changes.length ? '建议已应用，涉及的文档已更新' : '已确认任务完成');
    });
  }

  function exportDocument(entry: DocumentEntry, current: Buffer) {
    void perform(async () => {
      const title = current.title?.trim() || entry.title;
      const filename = (title.replace(/[\\/:"*?<>|]/g, '_') || '文档') + '.html';
      const destination = await saveDialog({
        title: '导出当前文档为 HTML', defaultPath: filename, filters: [{ name: 'HTML', extensions: ['html'] }],
      });
      if (!destination) return;
      const { renderHtml } = await import('./htmlExport');
      const html = await renderHtml(project.root, entry.path, title, current.draft, project.markdown);
      await projectApi.exportHtml(destination, html);
      setNotice('HTML 已导出');
    });
  }

  return (
    <div className="project-shell" hidden={!page} inert={locked}>
      {!assignment && (error || notice) && <p className={error ? 'project-banner error-banner' : 'project-banner'} role={error ? 'alert' : 'status'}>{error || notice}</p>}
      {page === 'overview' && (
        <div className="page-scroll project-content">
          <article className="page-content">
            <div className="page-intro">
              <span className="page-icon"><Folder /></span>
              <div><h2>{project.name}</h2><p className="document-path">{project.root}</p></div>
            </div>
            {(project.sourceCommit || project.baselineCommit) && <dl className="project-versions document-path">
              {project.sourceCommit && <><dt>Git 来源提交</dt><dd><code>{project.sourceCommit}</code></dd></>}
              {project.baselineCommit && <><dt>Studio 版本基线</dt><dd><code>{project.baselineCommit}</code></dd></>}
            </dl>}
            <div className="surface">
              {groups.slice(1).map(([group, label]) => {
                const count = project.documents.filter(entry => entry.kind === group).length;
                return count > 0 && <button key={group} type="button" className="detail-row detail-button" onClick={() => {
                  setFilter(group);
                  onNavigate('chapters');
                }}><FileText /><strong className="detail-title">{label}</strong><span>{count}</span></button>;
              })}
            </div>
            <div className="project-actions">
              <button className="text-button" type="button" disabled={busy} onClick={() => void perform(async () => {
                onUpdate(await projectApi.open(project.root));
                setNotice('已刷新文档目录');
              })}><RefreshCw />刷新目录</button>
            </div>
          </article>
        </div>
      )}
      {page === 'tasks' && <TaskReview tasks={tasks} selected={selectedTask} busy={busy} onSelect={setSelectedTask}
        onRefresh={refreshTasks} onLocate={locate} onReview={review} />}
      {editing && (
        <div className="chapter-workspace project-content">
          <aside className="document-index" aria-label="项目文档">
            <header className="index-heading">
              <h2>{page === 'chapters' ? '项目文档' : groups.find(([key]) => key === kind)?.[1]}</h2>
              <span>{entries.length}</span>
            </header>
            {page === 'chapters' && <select aria-label="物料类型" className="document-filter" value={filter} onChange={event => {
              setFilter(event.target.value as Filter);
              setSelection(null);
            }}>{groups.map(([key, label]) => <option key={key} value={key}>{label}</option>)}</select>}
            <ol className="index-list">
              {entries.map(item => {
                const draft = buffers[item.path];
                return (
                  <li key={item.id ?? item.path}>
                    <button type="button" className="index-item" disabled={busy} aria-pressed={item.path === path} onClick={() => select(item.path)}>
                      {item.kind === 'chapter' && <span className="chapter-order">{item.chapterNumber}</span>}
                      <span><strong>{draft?.title?.trim() || item.title}</strong>
                        {draft && isDirty(draft) && <small>未保存</small>}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ol>
          </aside>
          {path && buffer ? (
            <section className="reader project-reader" aria-label="Markdown 工作区" onKeyDown={event => {
              if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
                event.preventDefault();
                if (canSave && entry.kind !== 'generated') saveBuffer(buffer);
              }
            }}>
              <header className="reader-toolbar">
                {buffer.title !== null ? (
                  <label className="document-title-field">
                    <span>{entry.kind === 'chapter' ? '章名' : '标题'}</span>
                    <input ref={titleInput} value={buffer.title} readOnly={entry.kind === 'generated' || busy} onChange={event => {
                      const title = event.target.value;
                      setBuffers(old => ({ ...old, [path]: { ...buffer, title } }));
                    }} />
                  </label>
                ) : <strong className="editor-title">{entry.title}</strong>}
                <div className="segmented" role="group" aria-label="Markdown 视图">
                  <button type="button" aria-pressed={!preview} onClick={() => { setPreview(false); setSelection(null); }}><Code2 />{entry.kind === 'generated' ? '源码' : '编辑'}</button>
                  <button type="button" aria-pressed={preview} onClick={() => { setPreview(true); setSelection(null); }}><Eye />预览</button>
                </div>
              </header>
              {preview ? (
                <div className="reader-scroll"><article className="paper markdown-preview">
                  <ProjectMarkdown text={buffer.draft} format={project.markdown} components={{
                    a: ({ href, children }) => {
                      const hrefPath = projectPath(path, href);
                      const target = project.documents.find(entry => entry.path === hrefPath);
                      return target ? <button className="document-link" type="button" onClick={() => openEntry(target)}>{children}</button> : <span>{children}</span>;
                    },
                    img: ({ src, alt, title }) => {
                      const image = projectPath(path, src);
                      return <LocalImage key={image ?? src} root={project.root} path={image} alt={alt} title={title} />;
                    },
                  }} />
                </article></div>
              ) : (
                <textarea key={path} className="markdown-editor" aria-label="Markdown 正文" ref={editor}
                  value={buffer.draft} readOnly={entry.kind === 'generated' || busy} spellCheck={false}
                  onChange={event => {
                    const draft = event.target.value;
                    setBuffers(old => ({ ...old, [path]: { ...buffer, draft } }));
                    setSelection(null);
                  }}
                  onSelect={event => {
                    const { selectionStart: start, selectionEnd: end } = event.currentTarget;
                    setSelection(start === end ? null : { path, start, end });
                  }} />
              )}
              <footer className="reader-footer project-editor-actions">
                <div>
                  <button className="text-button" type="button" onClick={() => void reload()} disabled={busy}><RefreshCw />重新读取</button>
                  <button className="text-button" type="button" onClick={() => exportDocument(entry, buffer)} disabled={busy}><Download />导出 HTML</button>
                </div>
                {entry.kind === 'generated' ? <span>生成产物 · 只读</span> : (
                  <div>
                    <button className="text-button" type="button" disabled={busy || preview || !selectedText.trim()}
                      onClick={beginAssignment}><ClipboardPlus />交办选区</button>
                    <button className="primary-button" type="button" disabled={busy || !canSave}
                      onClick={() => saveBuffer(buffer)}><Save />保存</button>
                  </div>
                )}
              </footer>
            </section>
          ) : <div className="page-content empty-state">{path ? '正在读取 Markdown…' : '此分类还没有 Markdown 文档。'}</div>}
        </div>
      )}
      {assignment && (
        <dialog className="assignment-dialog" aria-labelledby="assignment-title" ref={dialog} onCancel={event => {
          event.preventDefault();
          if (!busy) setAssignment(null);
        }}>
          <form onSubmit={assign}>
            <h2 id="assignment-title">交办选区</h2>
            <p className="document-path"><code>{assignment.path}</code></p>
            <blockquote className="task-quote">{assignment.quote}</blockquote>
            <label htmlFor="assignment-instruction">任务要求</label>
            <textarea id="assignment-instruction" value={instruction} onChange={event => setInstruction(event.target.value)}
              required autoFocus readOnly={busy} placeholder="例如，核对这段对照世界观设定是否一致" />
            {error && <p className="project-banner error-banner" role="alert">{error}</p>}
            <footer className="proposal-actions">
              <button type="button" className="text-button" disabled={busy} onClick={() => setAssignment(null)}>取消</button>
              <button type="submit" className="primary-button" disabled={busy || !instruction.trim()}>
                {isDirty(buffers[assignment.path]) ? '保存并交办' : '交办任务'}
              </button>
            </footer>
          </form>
        </dialog>
      )}
    </div>
  );
}
