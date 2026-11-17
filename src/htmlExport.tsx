import { renderToStaticMarkup } from 'react-dom/server';
import { ProjectMarkdown, projectPath } from './ProjectMarkdown';
import { projectApi, type MarkdownFormat } from './project';

const styles = `
  :root { color-scheme: light; background: #f5f4f0; color: #202728; font: 16px/1.65 system-ui, sans-serif; }
  body { margin: 0; }
  main { max-width: 48rem; margin: auto; padding: clamp(1rem, 6vw, 4rem); background: white; }
  h1, h2, h3 { line-height: 1.25; }
  h1 { margin-top: 0; }
  a { color: #145b68; }
  img { display: block; max-width: 100%; height: auto; }
  table { border-collapse: collapse; }
  th, td { border: 1px solid #cdd5d5; padding: .4em .7em; }
  th, pre { background: #f2f5f5; }
  pre { padding: 1em; overflow: auto; }
  blockquote { border-inline-start: 3px solid #a0b5b7; margin-inline: 0; padding-inline: 1em; }
  @media print { :root { background: white; } main { max-width: none; padding: 0; } }
`;

export async function renderHtml(root: string, path: string, title: string, text: string, format: MarkdownFormat) {
  const images = new Map<string, string>();
  renderToStaticMarkup(<ProjectMarkdown text={text} format={format} components={{
    img: ({ src }) => {
      const image = projectPath(path, src);
      if (image) images.set(image, '');
      return null;
    },
  }} />);

  let totalImageBytes = 0;
  for (const image of images.keys()) {
    const data = await projectApi.image(root, image);
    totalImageBytes += data.length;
    if (totalImageBytes > 48 * 1024 * 1024) throw new Error('嵌入的图片超过 HTML 导出大小限制。');
    images.set(image, data);
  }

  const html = renderToStaticMarkup(<html>
    <head>
      <meta charSet="utf-8" />
      <meta name="viewport" content="width=device-width, initial-scale=1" />
      <meta httpEquiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; base-uri 'none'" />
      <title>{title}</title>
      <style>{styles}</style>
    </head>
    <body><main><ProjectMarkdown text={text} format={format} components={{
      img: ({ src, alt, title }) => {
        const data = images.get(projectPath(path, src) ?? '');
        return data ? <img src={data} alt={alt ?? ''} title={title} loading="lazy" /> : <span>{alt}</span>;
      },
      a: ({ href, children }) => href && /^(https?:|mailto:|#)/i.test(href)
        ? <a href={href}>{children}</a> : <span>{children}</span>,
    }} /></main></body>
  </html>);
  return html.startsWith('<!DOCTYPE html>') ? html : '<!DOCTYPE html>\n' + html;
}
