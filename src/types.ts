export interface ProjectMeta {
  name: string;
  description: string;
  engine: string; // "unity" | "unreal" | "godot" | "none"
  tags: string[];
  cover?: string | null;
  created: string;
  schemaVersion: number;
  tabs: CustomTab[];
}

/** An extra tab whose pages live in .projectmanager/<id>/. */
export interface CustomTab {
  id: string;
  title: string;
  instructions: string;
  preset: string;
}

export interface TabPreset {
  id: string;
  title: string;
  description: string;
  instructions: string;
}

export interface WorklogEntry {
  file: string;
  date: string;
  author: string;
  kind: string;
  title: string;
  body: string;
  /** True if the entry claimed a time later than its file was written; `date` is then the file's time. */
  dateCorrected: boolean;
  claimedDate: string;
}

export interface ProjectSummary {
  path: string;
  meta: ProjectMeta;
  coverUrl?: string | null;
  lastEntry?: WorklogEntry | null;
  missing: boolean;
}

export interface LaunchOption {
  id: string;
  label: string;
  available: boolean;
  hint: string;
  /** The program's own icon as a PNG data URL, when Windows could provide it. */
  icon?: string | null;
}

export interface ProjectDetail {
  summary: ProjectSummary;
  state: string;
  docs: string[];
  worklog: WorklogEntry[];
  launchers: LaunchOption[];
  aiFolders: string[];
  pointerTarget?: string | null;
  tabPages: Record<string, string[]>;
}

export interface Settings {
  vscode: string;
  intellij: string;
  unityEditorsDir: string;
  unrealEditor: string;
  godot: string;
}

export interface DetectedTools {
  vscode?: string | null;
  intellij?: string | null;
  unityEditorsDir?: string | null;
  unrealEditor?: string | null;
  godot?: string | null;
}

export interface NewWorklogEntry {
  author: string;
  kind: string;
  title: string;
  summary: string;
  changes: string;
  decisions: string;
  issues: string;
  nextSteps: string;
}

export const ENTRY_KINDS = ["feature", "fix", "refactor", "docs", "research", "setup", "other"];

export interface AiToolsInfo {
  pmPath?: string | null;
}

export interface ClientStatus {
  id: string; // "claude-desktop" | "claude-code"
  name: string;
  installed: boolean;
  connected: boolean;
  outdated: boolean;
  detail: string;
}

export interface SessionInfo {
  id: string;
  name?: string | null;
  summary?: string | null;
  firstPrompt?: string | null;
  started?: string | null;
  lastActivity?: string | null;
  messageCount: number;
  gitBranch?: string | null;
  sizeBytes: number;
}
