import { invoke } from '@tauri-apps/api/core';

export type DocumentKind = 'chapter' | 'summary' | 'worldbuilding' | 'writing' | 'illustration' | 'material' | 'generated';
export type MarkdownFormat = 'commonmark' | 'gfm';

export interface DocumentEntry {
  id: string | null;
  path: string;
  title: string;
  kind: DocumentKind;
  chapterNumber: number | null;
}

export interface Project {
  root: string;
  name: string;
  git: boolean;
  managed: boolean;
  preset: 'general' | 'manual' | 'notes' | 'novel';
  markdown: MarkdownFormat;
  sourceCommit: string | null;
  baselineCommit: string | null;
  documents: DocumentEntry[];
}

export interface ManagedProject {
  root: string;
  name: string;
  importedAt: number;
}

export type ImportKind = 'directory' | 'archive' | 'git';

export interface ProjectDocument {
  path: string;
  text: string;
  version: string;
  title: string | null;
  bodyStart: number;
  needsTitleMigration: boolean;
}

export interface Change {
  path: string;
  version: string;
  oldText: string;
  newText: string;
}

export interface Proposal {
  id: string;
  summary: string;
  changes: Change[];
  status: 'pending' | 'applied' | 'rejected';
}

export interface Task {
  id: string;
  path: string;
  instruction: string;
  anchor: { quote: string };
  proposals: Proposal[];
}

export interface Location {
  path: string;
  version: string;
  start: number;
  end: number;
}

interface NewTask {
  path: string;
  version: string;
  start: number;
  end: number;
  instruction: string;
}

export const projectApi = {
  open: (root: string) => invoke<Project>('open_project', { root }),
  list: () => invoke<ManagedProject[]>('list_managed_projects'),
  import: (source: string, kind: ImportKind) => invoke<Project>('import_project', { source, kind }),
  export: (root: string, destination: string) => invoke<void>('export_project', { root, destination }),
  read: (root: string, path: string) => invoke<ProjectDocument>('project_read', { root, path }),
  image: (root: string, path: string) => invoke<string>('project_image', { root, path }),
  exportHtml: (destination: string, html: string) => invoke<void>('export_html', { destination, html }),
  save: (root: string, document: ProjectDocument, text: string, title: string | null) =>
    invoke<ProjectDocument>('project_save', { root, path: document.path, version: document.version, text, title }),
  tasks: (root: string) => invoke<Task[]>('project_tasks', { root }),
  createTask: (root: string, input: NewTask) => invoke<Task>('create_task', { root, input }),
  locate: (root: string, taskId: string) => invoke<Location | null>('locate_task', { root, taskId }),
  review: (root: string, taskId: string, proposalId: string, apply: boolean) =>
    invoke<void>('review_proposal', { root, taskId, proposalId, apply }),
  mcpProgram: () => invoke<string | null>('mcp_program'),
};

export const taskComplete = (task: Task) => task.proposals.some(proposal => proposal.status === 'applied');
