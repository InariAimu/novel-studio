import { ChevronRight, Download, Folder, FolderInput, GitBranch, PackageOpen } from 'lucide-react';
import type { ImportKind, ManagedProject, Project } from './project';

interface Props {
  active: Project | null;
  projects: ManagedProject[];
  busy: boolean;
  onImport: (kind: ImportKind) => void;
  onOpen: (root: string) => void;
  onExport: () => void;
}

const importedDate = new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'medium' });

export default function ProjectLibrary({ active, projects, busy, onImport, onOpen, onExport }: Props) {
  return (
    <div className="page-scroll">
      <div className="page-content library-content">
        <div className="library-actions">
          <button type="button" className="primary-button" disabled={busy} onClick={() => onImport('directory')}>
            <FolderInput />导入目录
          </button>
          <button type="button" className="text-button" disabled={busy} onClick={() => onImport('archive')}>
            <PackageOpen />导入项目包
          </button>
          <button type="button" className="text-button" disabled={busy} onClick={() => onImport('git')}>
            <GitBranch />从 Git 导入
          </button>
        </div>
        <p className="empty-state">导入会在 Studio 项目库创建独立副本。从 Git 导入使用本地仓库的当前提交。</p>
        {active && (
          <section className="library-section" aria-label="当前打开的项目">
            <h2 className="section-title">当前打开的项目</h2>
            <div className="surface detail-row library-active">
              <Folder aria-hidden="true" />
              <div className="detail-title"><strong>{active.name}</strong><small>{active.managed ? 'Studio 托管' : '外部目录'}</small></div>
              <button type="button" className="text-button" disabled={busy} onClick={onExport}><Download />导出项目包</button>
            </div>
          </section>
        )}
        <section className="library-section" aria-label="托管项目">
          <h2 className="section-title">托管项目</h2>
          {projects.length ? (
            <div className="surface">
              {projects.map(project => (
                <button key={project.root} type="button" className="detail-row detail-button" disabled={busy}
                  onClick={() => onOpen(project.root)}>
                  <Folder aria-hidden="true" />
                  <span className="detail-title"><strong>{project.name}</strong><small>导入于 {importedDate.format(project.importedAt)}</small></span>
                  <ChevronRight aria-hidden="true" />
                </button>
              ))}
            </div>
          ) : <p className="empty-state">还没有托管项目。</p>}
        </section>
      </div>
    </div>
  );
}
