import { BookOpen, ChevronRight, Folder, Image, Moon, Plug, ScrollText, Sun, Monitor, type LucideIcon } from 'lucide-react';
import { useState } from 'react';
import type { Theme, WorkspacePage } from './App';
import { chapters, illustrationPrompts, projectName, writingPrompts } from './demo';

const projectSections = [
  { page: 'chapters', title: '章节', directory: 'manuscript/', count: `${chapters.length - 1} 章`, icon: BookOpen },
  { page: 'writing', title: '写作提示词', directory: 'prompts/writing/', count: `${writingPrompts.length} 条`, icon: ScrollText },
  { page: 'illustration', title: '插图提示词', directory: 'prompts/illustration/', count: `${illustrationPrompts.length} 条`, icon: Image },
] as const;

export function Overview({ onNavigate }: { onNavigate: (page: WorkspacePage) => void }) {
  return (
    <div className="page-scroll">
      <div className="page-content">
        <div className="page-intro">
          <span className="page-icon"><Folder aria-hidden="true" /></span>
          <div><h2>{projectName}</h2><p>内置示例项目</p></div>
        </div>
        <section aria-labelledby="project-files-heading">
          <h3 className="section-title" id="project-files-heading">项目内容</h3>
          <div className="surface">
            {projectSections.map(({ page, title, directory, count, icon: Icon }) => (
              <button type="button" className="detail-row detail-button" key={page} onClick={() => onNavigate(page)}>
                <Icon aria-hidden="true" />
                <span className="detail-title"><strong>{title}</strong><code>{directory}</code></span>
                <span>{count}</span><ChevronRight aria-hidden="true" />
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}

export function PromptLibrary({ prompts, icon: Icon }: { prompts: typeof writingPrompts; icon: LucideIcon }) {
  const [selected, setSelected] = useState(0);
  const prompt = prompts[selected];

  return (
    <div className="chapter-workspace">
      <aside className="document-index" aria-labelledby="prompt-index-heading">
        <header className="index-heading"><h2 id="prompt-index-heading">提示词列表</h2><span>{prompts.length} 条</span></header>
        <ul className="index-list">
          {prompts.map((entry, index) => (
            <li key={entry.file}>
              <button type="button" className="index-item" aria-pressed={index === selected} onClick={() => setSelected(index)}>
                <Icon aria-hidden="true" /><strong>{entry.title}</strong>
              </button>
            </li>
          ))}
        </ul>
      </aside>
      <div className="page-scroll" key={prompt.file}>
        <article className="page-content">
          <h2>{prompt.title}</h2>
          <p className="document-path"><code>{prompt.file}</code></p>
          <div className="surface prompt-body">{prompt.body}</div>
        </article>
      </div>
    </div>
  );
}

export function ConnectionPage() {
  return (
    <div className="page-scroll">
      <div className="page-content">
        <div className="page-intro">
          <span className="page-icon"><Plug aria-hidden="true" /></span>
          <div><h2>本地 MCP 服务</h2><p>待实现</p></div>
        </div>
        <section aria-labelledby="client-heading">
          <h3 className="section-title" id="client-heading">计划支持的客户端</h3>
          <div className="surface">
            <div className="detail-row"><strong className="detail-title">Codex</strong><code>stdio</code></div>
            <div className="detail-row"><strong className="detail-title">DeepSeek Harness（dsh）</strong><code>stdio</code></div>
          </div>
        </section>
      </div>
    </div>
  );
}

const themes = [
  { id: 'light', label: '浅色', icon: Sun },
  { id: 'dark', label: '深色', icon: Moon },
  { id: 'system', label: '跟随系统', icon: Monitor },
] as const;

export function Settings({ theme, onThemeChange }: { theme: Theme; onThemeChange: (theme: Theme) => void }) {
  return (
    <div className="page-scroll">
      <section className="page-content" aria-labelledby="appearance-heading">
        <h2 id="appearance-heading">外观</h2>
        <div className="surface settings-surface">
          <div className="detail-row settings-row">
            <strong className="detail-title" id="theme-label">界面主题</strong>
            <div className="segmented" role="group" aria-labelledby="theme-label">
              {themes.map(({ id, icon: Icon, label }) => (
                <button type="button" key={id} aria-pressed={id === theme} onClick={() => onThemeChange(id)}>
                  <Icon aria-hidden="true" />{label}
                </button>
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
