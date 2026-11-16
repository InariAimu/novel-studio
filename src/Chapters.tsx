import { BookOpen, ChevronLeft, ChevronRight, Code2, FileText } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { chapters, projectName } from './demo';

export default function Chapters() {
  const [selected, setSelected] = useState(1);
  const [mode, setMode] = useState<'preview' | 'source'>('preview');
  const reader = useRef<HTMLDivElement>(null);
  const chapter = chapters[selected];

  useEffect(() => {
    reader.current?.scrollTo(0, 0);
  }, [selected, mode]);

  return (
    <div className="chapter-workspace">
      <aside className="document-index" aria-labelledby="chapter-index-heading">
        <header className="index-heading">
          <h2 id="chapter-index-heading">章节目录</h2>
          <span>{chapters.length - 1} 章</span>
        </header>
        <ol className="index-list">
          {chapters.map((entry, index) => (
            <li key={entry.file}>
              <button type="button" className="index-item" aria-pressed={index === selected} onClick={() => setSelected(index)}>
                <FileText aria-hidden="true" />
                <span><small>{entry.label}</small><strong>{entry.title}</strong></span>
              </button>
            </li>
          ))}
        </ol>
      </aside>
      <section className="reader" aria-label="章节阅读区">
        <header className="reader-toolbar">
          <span className="file-path" title={`manuscript/${chapter.file}`}><FileText aria-hidden="true" />{chapter.file}</span>
          <div className="segmented" role="group" aria-label="章节视图">
            <button type="button" aria-pressed={mode === 'preview'} onClick={() => setMode('preview')}><BookOpen aria-hidden="true" />预览</button>
            <button type="button" aria-pressed={mode === 'source'} onClick={() => setMode('source')}><Code2 aria-hidden="true" />源码</button>
          </div>
        </header>
        <div className="reader-scroll" ref={reader}>
          {mode === 'preview' ? (
            <article className="paper">
              <header className="chapter-heading">
                <p>{chapter.label}</p>
                <h2>{chapter.title}</h2>
              </header>
              <div className="chapter-prose">
                {chapter.paragraphs.map(paragraph => <p key={paragraph}>{paragraph}</p>)}
              </div>
            </article>
          ) : (
            <pre className="paper chapter-source"><code>{[
              selected === 0 ? `# ${projectName}` : `## ${chapter.label} ${chapter.title}`,
              ...chapter.paragraphs,
            ].join('\n\n')}</code></pre>
          )}
        </div>
        <footer className="reader-footer">
          <button type="button" className="text-button" disabled={selected === 0} onClick={() => setSelected(selected - 1)}>
            <ChevronLeft aria-hidden="true" />{selected === 1 ? '卷首' : '上一章'}
          </button>
          <button type="button" className="text-button" disabled={selected === chapters.length - 1} onClick={() => setSelected(selected + 1)}>
            下一章<ChevronRight aria-hidden="true" />
          </button>
        </footer>
      </section>
    </div>
  );
}
