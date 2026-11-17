import { diffLines } from 'diff';
import { Check, ClipboardList, FileText, LocateFixed, RefreshCw, X } from 'lucide-react';
import type { Proposal, Task } from './project';
import { taskComplete } from './project';

interface Props {
  tasks: Task[];
  selected: string | null;
  busy: boolean;
  onSelect: (id: string) => void;
  onRefresh: () => void;
  onLocate: (task: Task) => void;
  onReview: (task: Task, proposal: Proposal, apply: boolean) => void;
}

const status = { pending: '待审阅', applied: '已应用', rejected: '已退回' };

export default function TaskReview({ tasks, selected, busy, onSelect, onRefresh, onLocate, onReview }: Props) {
  const task = tasks.find(entry => entry.id === selected) ?? tasks.at(-1);

  return (
    <div className="chapter-workspace project-content">
      <aside className="document-index" aria-label="任务目录">
        <header className="index-heading">
          <h2>段落任务</h2>
          <button className="text-button" type="button" onClick={onRefresh} disabled={busy} aria-label="刷新任务"><RefreshCw /></button>
        </header>
        <ul className="index-list">
          {[...tasks].reverse().map(entry => (
            <li key={entry.id}>
              <button className="index-item" type="button" aria-pressed={entry.id === task?.id} onClick={() => onSelect(entry.id)}>
                <ClipboardList aria-hidden="true" />
                <span>
                  <strong className="task-list-title">{entry.instruction}</strong>
                  <small>{taskComplete(entry) ? '已完成' : entry.proposals.some(item => item.status === 'pending') ? '待审阅' : '待处理'}</small>
                </span>
              </button>
            </li>
          ))}
        </ul>
      </aside>
      <div className="page-scroll">
        {task ? (
          <article className="page-content task-review">
            <header className="task-heading">
              <div><p className="document-path"><FileText aria-hidden="true" /><code>{task.path}</code></p><h2>段落任务</h2></div>
              <button className="text-button" type="button" onClick={() => onLocate(task)} disabled={busy}><LocateFixed />定位原文</button>
            </header>
            <p className="task-instruction">{task.instruction}</p>
            <blockquote className="task-quote">{task.anchor.quote}</blockquote>
            <h3 className="section-title">改动建议</h3>
            {task.proposals.length === 0 && <p className="empty-state">外部 harness 提交的建议会显示在这里。</p>}
            {[...task.proposals].reverse().map(proposal => (
              <section className="surface proposal-card" key={proposal.id} aria-label="改动建议">
                <header><span className="proposal-status">{status[proposal.status]}</span><p>{proposal.summary}</p></header>
                {proposal.changes.map(change => (
                  <div className="proposal-change" key={change.path}>
                    <code className="document-path">{change.path}</code>
                    <pre className="proposal-diff" aria-label={change.path + ' 改动对比'}>
                      {diffLines(change.oldText, change.newText).flatMap((part, index) =>
                        part.value.replace(/\n$/, '').split('\n').map((line, lineIndex) => (
                          <span key={index + '-' + lineIndex} className={part.added ? 'diff-add' : part.removed ? 'diff-remove' : undefined}>
                            {part.added ? '+' : part.removed ? '−' : ' '} {line}
                          </span>
                        ))
                      )}
                    </pre>
                  </div>
                ))}
                {proposal.status === 'pending' && (
                  <footer className="proposal-actions">
                    <button className="text-button" type="button" disabled={busy} onClick={() => onReview(task, proposal, false)}><X />退回建议</button>
                    {!taskComplete(task) && (
                      <button className="primary-button" type="button" disabled={busy} onClick={() => onReview(task, proposal, true)}>
                        <Check />{proposal.changes.length ? '应用建议' : '确认完成'}
                      </button>
                    )}
                  </footer>
                )}
              </section>
            ))}
          </article>
        ) : <div className="page-content"><h2>任务与审阅</h2><p className="empty-state">在 Markdown 编辑器中选中文本，再点击「交办选区」。</p></div>}
      </div>
    </div>
  );
}
