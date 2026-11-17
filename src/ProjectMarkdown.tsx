import { useEffect, useState } from 'react';
import Markdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { projectApi, type MarkdownFormat } from './project';

export function ProjectMarkdown({ text, format, components }: {
  text: string;
  format: MarkdownFormat;
  components: Components;
}) {
  return <Markdown remarkPlugins={format === 'gfm' ? [remarkGfm] : []} skipHtml components={components}>
    {text}
  </Markdown>;
}

export function projectPath(documentPath: string, reference?: string) {
  if (!reference || /^(?:[a-z][a-z0-9+.-]*:|\/\/|\\|[#?])/i.test(reference)) return undefined;
  try {
    const base = new URL('https://project.invalid/');
    base.pathname = '/' + documentPath.split('/').map(encodeURIComponent).join('/');
    const target = new URL(reference, base);
    return target.origin === base.origin ? decodeURIComponent(target.pathname.slice(1)) : undefined;
  } catch { return undefined; }
}

export function LocalImage({ root, path, alt, title }: {
  root: string;
  path?: string;
  alt?: string;
  title?: string;
}) {
  const [result, setResult] = useState<{ data?: string; error?: string }>();
  useEffect(() => {
    if (!path) return;
    let active = true;
    void projectApi.image(root, path).then(data => {
      if (active) setResult({ data });
    }).catch(error => {
      if (active) setResult({ error: String(error) });
    });
    return () => { active = false; };
  }, [root, path]);

  return result?.data
    ? <img src={result.data} alt={alt ?? ''} title={title} />
    : <span title={result?.error ?? title}>{alt}</span>;
}
