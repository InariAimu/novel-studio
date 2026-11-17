import { BookOpen, Check, ChevronRight, Copy, Folder, Image, Plug, ScrollText, type LucideIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { Theme, WorkspacePage } from './App';
import { chapters, illustrationPrompts, projectName, writingPrompts } from './demo';
import { projectApi } from './project';

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

export function ConnectionPage({ root }: { root?: string }) {
  const [program, setProgram] = useState<string | null>(null);
  const [message, setMessage] = useState('');
  useEffect(() => {
    if (!root) return;
    let active = true;
    void projectApi.mcpProgram().then(value => {
      if (active) setProgram(value);
    }).catch(reason => { if (active) setMessage(String(reason)); });
    return () => { active = false; };
  }, [root]);
  const config = root && program ? JSON.stringify({ command: program, args: ['--project', root] }, null, 2) : '';

  return (
    <div className="page-scroll">
      <div className="page-content">
        <div className="page-intro">
          <span className="page-icon"><Plug aria-hidden="true" /></span>
          <div><h2>本地 MCP 服务</h2><p>由外部 harness 通过 stdio 启动</p></div>
        </div>
        {config ? (
          <section aria-labelledby="client-heading">
            <h3 className="section-title" id="client-heading">当前项目的连接配置</h3>
            <textarea className="mcp-config" aria-label="MCP 连接配置" readOnly value={config} />
            <div className="project-actions">
              <button className="primary-button" type="button" onClick={() => {
                void navigator.clipboard.writeText(config).then(() => setMessage('已复制配置'))
                  .catch(() => setMessage('请选择配置文本并复制。'));
              }}><Copy />复制配置</button>
              <span role="status">{message}</span>
            </div>
          </section>
        ) : <p className="empty-state">{message || (root ? '请将 novel-studio-mcp 放在桌面程序旁边，再打开此页。' : '打开项目后可查看它的连接配置。')}</p>}
      </div>
    </div>
  );
}

export const accentColors = [
  { color: '#f97316', label: '橙色' },
  { color: '#3b82f6', label: '蓝色' },
  { color: '#10b981', label: '绿色' },
  { color: '#8b5cf6', label: '紫色' },
  { color: '#e85b73', label: '玫红色' },
] as const;

export type AccentColor = (typeof accentColors)[number]['color'];

export function Settings({ theme, accent, onThemeChange, onAccentChange }: {
  theme: Theme;
  accent: AccentColor;
  onThemeChange: (theme: Theme) => void;
  onAccentChange: (accent: AccentColor) => void;
}) {
  return (
    <div className="page-scroll">
      <section className="page-content" aria-labelledby="appearance-heading">
        <h2 id="appearance-heading">界面外观</h2>
        <div className="surface appearance-settings">
          <label className="appearance-field">
            <span>主题</span>
            <select value={theme} onChange={event => onThemeChange(event.currentTarget.value as Theme)}>
              <option value="system">跟随系统</option>
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </label>
          <div className="appearance-field">
            <span id="accent-label">强调色</span>
            <div className="accent-options" role="group" aria-labelledby="accent-label">
              {accentColors.map(({ color, label }) => (
                <button type="button" key={color} title={label} aria-label={label} aria-pressed={color === accent}
                  style={{ backgroundColor: color }} onClick={() => onAccentChange(color)}>
                  {color === accent && <Check aria-hidden="true" />}
                </button>
              ))}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
