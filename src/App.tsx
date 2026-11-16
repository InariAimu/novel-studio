import {
  BookOpen, ChevronRight, FileText, Folder, FolderOpen, Image, LayoutGrid,
  Plug, ScrollText, Settings2,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import Chapters from './Chapters';
import { chapters, illustrationPrompts, projectName, writingPrompts } from './demo';
import { ConnectionPage, Overview, PromptLibrary, Settings } from './WorkspacePages';
import TitleBar from './TitleBar';

const workspacePages = [
  { id: 'overview', label: '项目总览', icon: LayoutGrid },
  { id: 'chapters', label: '章节', icon: BookOpen },
  { id: 'writing', label: '写作提示词', icon: ScrollText },
  { id: 'illustration', label: '插图提示词', icon: Image },
] as const;

export type WorkspacePage = (typeof workspacePages)[number]['id'];
type Page = WorkspacePage | 'mcp' | 'settings';
export type Theme = 'light' | 'dark' | 'system';

function getInitialTheme(): Theme {
  try {
    const theme = localStorage.getItem('novel-studio.theme');
    if (theme === 'dark' || theme === 'system') return theme;
  } catch { /* The shell also works without browser storage. */ }
  return 'light';
}

export default function App() {
  const [page, setPage] = useState<Page>('chapters');
  const [theme, setTheme] = useState<Theme>(getInitialTheme);
  const workspacePage = workspacePages.find(entry => entry.id === page);
  const pageTitle = workspacePage?.label ?? (page === 'mcp' ? 'MCP 接入' : '设置');

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem('novel-studio.theme', theme);
    } catch { /* Keep the current preference for this session. */ }
  }, [theme]);

  return (
    <div className="app-shell">
      <TitleBar />
      <aside className="sidebar">
        <div className="sidebar-main">
          <nav aria-label="工作区">
            <h2 className="sidebar-label">工作区</h2>
            {workspacePages.map(({ id, label, icon: Icon }) => (
              <button key={id} type="button" className="nav-item" aria-current={id === page ? 'page' : undefined} onClick={() => setPage(id)}>
                <Icon aria-hidden="true" /><span>{label}</span>
              </button>
            ))}
          </nav>
          <section className="project-section" aria-labelledby="current-project-heading">
            <h2 className="sidebar-label" id="current-project-heading">当前项目</h2>
            <button type="button" className="project-entry" onClick={() => setPage('overview')}>
              <span className="project-mark"><FolderOpen aria-hidden="true" /></span>
              <span><strong>{projectName}</strong><small>内置示例</small></span>
              <ChevronRight className="project-arrow" aria-hidden="true" />
            </button>
          </section>
        </div>
        <nav className="sidebar-bottom" aria-label="应用">
          <button type="button" className="nav-item" aria-current={page === 'mcp' ? 'page' : undefined} onClick={() => setPage('mcp')}>
            <Plug aria-hidden="true" /><span>MCP 接入</span>
          </button>
          <button type="button" className="nav-item" aria-current={page === 'settings' ? 'page' : undefined} onClick={() => setPage('settings')}>
            <Settings2 aria-hidden="true" /><span>设置</span>
          </button>
        </nav>
      </aside>
      <main className="main-panel">
        <header className="workspace-header">
          {workspacePage && <div className="project-crumb"><Folder aria-hidden="true" /><span>{projectName}</span><ChevronRight aria-hidden="true" /></div>}
          <h1>{pageTitle}</h1>
        </header>
        {page === 'overview' && <Overview onNavigate={setPage} />}
        {page === 'chapters' && <Chapters />}
        {page === 'writing' && <PromptLibrary key={page} prompts={writingPrompts} icon={ScrollText} />}
        {page === 'illustration' && <PromptLibrary key={page} prompts={illustrationPrompts} icon={Image} />}
        {page === 'mcp' && <ConnectionPage />}
        {page === 'settings' && <Settings theme={theme} onThemeChange={setTheme} />}
      </main>
      <footer className="statusbar">
        <span><span className="status-dot" />示例工作区</span>
        {page === 'chapters' && <span className="status-detail"><FileText aria-hidden="true" />Markdown · {chapters.length - 1} 章</span>}
        {page !== 'chapters' && <span className="status-detail">v0.1.0</span>}
      </footer>
    </div>
  );
}
