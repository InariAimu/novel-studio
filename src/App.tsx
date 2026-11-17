import {
  BookOpen, ChevronRight, ClipboardList, FileText, Folder, FolderOpen, Image, Library,
  Plug, ScrollText, Settings2,
} from 'lucide-react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { ask, open, save } from '@tauri-apps/plugin-dialog';
import { useEffect, useRef, useState } from 'react';
import Chapters from './Chapters';
import { chapters, illustrationPrompts, projectName, writingPrompts } from './demo';
import { projectApi, type ImportKind, type ManagedProject, type Project } from './project';
import ProjectLibrary from './ProjectLibrary';
import ProjectWorkspace from './ProjectWorkspace';
import { accentColors, ConnectionPage, Overview, PromptLibrary, Settings, type AccentColor } from './WorkspacePages';
import TitleBar from './TitleBar';

const projectPages = [
  { id: 'chapters', label: '章节', icon: BookOpen },
  { id: 'tasks', label: '任务与审阅', icon: ClipboardList },
  { id: 'writing', label: '写作提示词', icon: ScrollText },
  { id: 'illustration', label: '插图提示词', icon: Image },
] as const;

export type WorkspacePage = 'overview' | (typeof projectPages)[number]['id'];
type Page = WorkspacePage | 'library' | 'mcp' | 'settings';
export type Theme = 'light' | 'dark' | 'system';
const zipFilters = [{ name: 'Novel Studio 项目包', extensions: ['zip'] }];

function getInitialTheme(): Theme {
  try {
    const theme = localStorage.getItem('novel-studio.theme');
    if (theme === 'system' || theme === 'light' || theme === 'dark') return theme;
  } catch { /* The shell also works without browser storage. */ }
  return 'system';
}

function getInitialAccent(): AccentColor {
  try {
    const saved = localStorage.getItem('novel-studio.accent');
    const accent = accentColors.find(({ color }) => color === saved);
    if (accent) return accent.color;
  } catch { /* The shell also works without browser storage. */ }
  return accentColors[0].color;
}

export default function App() {
  const desktop = isTauri();
  const [page, setPage] = useState<Page>(desktop ? 'library' : 'overview');
  const [projectExpanded, setProjectExpanded] = useState(false);
  const [theme, setTheme] = useState<Theme>(getInitialTheme);
  const [accent, setAccent] = useState<AccentColor>(getInitialAccent);
  const [project, setProject] = useState<Project | null>(null);
  const [projects, setProjects] = useState<ManagedProject[]>([]);
  const [operation, setOperation] = useState('');
  const [projectError, setProjectError] = useState('');
  const [projectNotice, setProjectNotice] = useState('');
  const [dirty, setDirty] = useState(false);
  const dirtyRef = useRef(false);
  dirtyRef.current = dirty;
  const name = project?.name ?? projectName;
  const isNovel = !project || project.preset === 'novel';
  const chapterCount = project
    ? project.documents.filter(entry => entry.chapterNumber !== null).length
    : chapters.length - 1;
  const projectPage = projectPages.find(entry => entry.id === page);
  const pageTitle = page === 'overview' ? '项目总览' : page === 'chapters' && !isNovel ? '文档'
    : projectPage?.label ?? (page === 'library' ? '项目库' : page === 'mcp' ? 'MCP 接入' : '设置');

  useEffect(() => {
    if (!desktop) return;
    let active = true;
    const root = localStorage.getItem('novel-studio.project');
    if (root) {
      setOperation('正在打开项目…');
      void projectApi.open(root).then(value => {
        if (active) {
          setProject(value);
          setPage('overview');
        }
      }).catch(reason => {
        if (active) setProjectError(String(reason));
      }).finally(() => { if (active) setOperation(''); });
    }
    const listener = getCurrentWindow().onCloseRequested(async event => {
      if (!dirtyRef.current) return;
      try {
        if (!await ask('有文档尚未保存。确定关闭并放弃这些改动？', { title: '关闭 Novel Studio' })) event.preventDefault();
      } catch {
        event.preventDefault();
      }
    });
    return () => {
      active = false;
      void listener.then(unlisten => unlisten()).catch(() => {});
    };
  }, [desktop]);

  useEffect(() => {
    if (!desktop || page !== 'library') return;
    let active = true;
    void projectApi.list().then(items => {
      if (active) setProjects(items);
    }).catch(reason => {
      if (active) setProjectError(String(reason));
    });
    return () => { active = false; };
  }, [desktop, page]);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.setProperty('--accent-base', accent);
    try {
      localStorage.setItem('novel-studio.theme', theme);
      localStorage.setItem('novel-studio.accent', accent);
    } catch { /* Keep the current preference for this session. */ }
  }, [theme, accent]);

  async function runProjectAction(label: string, action: () => Promise<void>) {
    if (operation) return;
    setOperation(label);
    setProjectError('');
    setProjectNotice('');
    try {
      await action();
    } catch (reason) {
      setProjectError(String(reason));
    } finally {
      setOperation('');
    }
  }

  async function canSwitchProject() {
    return !dirtyRef.current || await ask('切换项目会放弃尚未保存的改动。继续？', { title: '切换项目' });
  }

  function navigateProject(next: WorkspacePage) {
    if (next !== 'overview') setProjectExpanded(true);
    setPage(next);
  }

  function activateProject(next: Project) {
    localStorage.setItem('novel-studio.project', next.root);
    setProject(next);
    setDirty(false);
    setProjectExpanded(false);
    setPage('overview');
  }

  function openProject(root?: string) {
    void runProjectAction('正在打开项目…', async () => {
      const selected = root ?? await open({ directory: true, multiple: false, title: '打开项目目录' });
      if (typeof selected !== 'string') return;
      if (selected === project?.root) {
        setPage('overview');
      } else if (await canSwitchProject()) {
        activateProject(await projectApi.open(selected));
      }
    });
  }

  function importProject(kind: ImportKind) {
    void runProjectAction('正在导入项目…', async () => {
      const source = await open(kind === 'archive'
        ? { filters: zipFilters, title: '选择要导入的 Studio 项目包' }
        : { directory: true, title: kind === 'git' ? '选择 Git 仓库根目录（导入当前 HEAD）' : '选择要导入的项目目录' });
      if (typeof source !== 'string' || !await canSwitchProject()) return;
      activateProject(await projectApi.import(source, kind));
      setProjectNotice('项目已导入 Studio 项目库');
    });
  }

  function exportProject() {
    if (!project) return;
    void runProjectAction('正在导出项目…', async () => {
      if (dirtyRef.current) {
        setProjectError('请先保存未保存的文档，再导出项目。');
        return;
      }
      const filename = project.name.replace(/[\\/:"*?<>|]/g, '_');
      const destination = await save({ filters: zipFilters, defaultPath: filename + '.zip', title: '导出 Studio 项目包' });
      if (destination) {
        await projectApi.export(project.root, destination);
        setProjectNotice('项目包已导出');
      }
    });
  }

  return (
    <div className="app-shell">
      <TitleBar />
      <aside className="sidebar">
        <div className="sidebar-main">
          <nav aria-label="项目导航">
            {desktop && <button type="button" className="nav-item" aria-current={page === 'library' ? 'page' : undefined}
              onClick={() => setPage('library')}><Library aria-hidden="true" /><span>项目库</span></button>}
            <div className="project-branch">
              <div className="project-row">
                <button type="button" className="project-entry nav-item" title={name}
                  aria-current={page === 'overview' ? 'page' : undefined} onClick={() => navigateProject('overview')}>
                  <Folder aria-hidden="true" /><span>{name}</span>
                </button>
                <button type="button" className="project-toggle" aria-label={projectExpanded ? '收起项目' : '展开项目'}
                  aria-expanded={projectExpanded} aria-controls="project-pages" onClick={() => setProjectExpanded(expanded => !expanded)}>
                  <ChevronRight aria-hidden="true" />
                </button>
              </div>
              <div id="project-pages" className="project-pages" hidden={!projectExpanded}>
                {projectPages.filter(({ id }) => (id !== 'tasks' || project)
                  && (isNovel || (id !== 'writing' && id !== 'illustration'))).map(({ id, label, icon: Icon }) => (
                  <button key={id} type="button" className="nav-item" aria-current={id === page ? 'page' : undefined}
                    onClick={() => navigateProject(id)}>
                    <Icon aria-hidden="true" /><span>{id === 'chapters' && !isNovel ? '文档' : label}</span>
                  </button>
                ))}
              </div>
            </div>
          </nav>
        </div>
        <nav className="sidebar-bottom" aria-label="应用">
          <button type="button" className="nav-item" aria-current={page === 'mcp' ? 'page' : undefined} onClick={() => setPage('mcp')}>
            <Plug aria-hidden="true" /><span>MCP 接入</span>
          </button>
          <button type="button" className="nav-item" aria-current={page === 'settings' ? 'page' : undefined} onClick={() => setPage('settings')}>
            <Settings2 aria-hidden="true" /><span>设置</span>
          </button>
        </nav>
        <div className="sidebar-status">
          <span><span className="status-dot" aria-hidden="true" />{dirty ? '有未保存改动' : project?.managed ? 'Studio 托管' : project ? '本地项目' : '示例工作区'}</span>
          {page === 'chapters' ? <span className="status-detail"><FileText aria-hidden="true" />
            Markdown · {isNovel ? `${chapterCount} 章` : `${project?.documents.length} 个文档`}
          </span> : <span>v0.1.0</span>}
        </div>
      </aside>
      <main className="main-panel">
        <header className="workspace-header">
          {(projectPage || page === 'overview') && <div className="project-crumb"><Folder aria-hidden="true" /><span>{name}</span><ChevronRight aria-hidden="true" /></div>}
          <h1>{pageTitle}</h1>
          {desktop && <button type="button" className="text-button open-project" disabled={!!operation} onClick={() => openProject()}>
            <FolderOpen />打开项目
          </button>}
        </header>
        <div className="workspace-body">
          {projectError && <p className="project-banner error-banner" role="alert">{projectError}</p>}
          {(operation || projectNotice) && <p className="project-banner" role="status">{operation || projectNotice}</p>}
          {project && <ProjectWorkspace key={project.root} project={project} locked={!!operation}
            page={page === 'settings' || page === 'mcp' || page === 'library' ? null : page}
            onNavigate={navigateProject} onUpdate={updated => setProject(current => current?.root === updated.root ? updated : current)}
            onDirty={setDirty} />}
          {!project && page === 'overview' && <Overview onNavigate={navigateProject} />}
          {!project && page === 'chapters' && <Chapters />}
          {!project && page === 'writing' && <PromptLibrary key={page} prompts={writingPrompts} icon={ScrollText} />}
          {!project && page === 'illustration' && <PromptLibrary key={page} prompts={illustrationPrompts} icon={Image} />}
          {page === 'library' && <ProjectLibrary active={project} projects={projects} busy={!!operation}
            onImport={importProject} onOpen={openProject} onExport={exportProject} />}
          {page === 'mcp' && <ConnectionPage root={project?.root} />}
          {page === 'settings' && <Settings theme={theme} accent={accent} onThemeChange={setTheme} onAccentChange={setAccent} />}
        </div>
      </main>
    </div>
  );
}
