// Thin typed wrappers around the Rust commands in src-tauri/src/lib.rs.
import { invoke } from "@tauri-apps/api/core";
import type {
  AiToolsInfo,
  ClientStatus,
  CustomTab,
  DetectedTools,
  NewWorklogEntry,
  ProjectDetail,
  ProjectMeta,
  ProjectSummary,
  SessionInfo,
  TabPreset,
  Settings,
  WorklogEntry,
} from "./types";

export const api = {
  listProjects: () => invoke<ProjectSummary[]>("list_projects"),
  addProject: (path: string) => invoke<ProjectSummary>("add_project", { path }),
  removeProject: (path: string) => invoke<void>("remove_project", { path }),
  getProject: (path: string) => invoke<ProjectDetail>("get_project", { path }),
  updateMeta: (path: string, meta: ProjectMeta) => invoke<void>("update_meta", { path, meta }),
  saveState: (path: string, content: string) => invoke<void>("save_state", { path, content }),
  readPage: (path: string, section: string, name: string) =>
    invoke<string>("read_page", { path, section, name }),
  savePage: (path: string, section: string, name: string, content: string) =>
    invoke<void>("save_page", { path, section, name, content }),
  createPage: (path: string, section: string, title: string) =>
    invoke<string>("create_page", { path, section, title }),
  tabPresets: () => invoke<TabPreset[]>("tab_presets"),
  addTab: (path: string, title: string, preset: string, instructions: string) =>
    invoke<CustomTab>("add_tab", { path, title, preset, instructions }),
  updateTab: (path: string, id: string, title: string, instructions: string) =>
    invoke<CustomTab>("update_tab", { path, id, title, instructions }),
  removeTab: (path: string, id: string) => invoke<void>("remove_tab", { path, id }),
  addWorklogEntry: (path: string, entry: NewWorklogEntry) =>
    invoke<WorklogEntry>("add_worklog_entry", { path, entry }),
  setCover: (path: string, source: string) => invoke<string>("set_cover", { path, source }),
  installAiPointer: (path: string, target: string | null) =>
    invoke<string[]>("install_ai_pointer", { path, target }),
  launchTool: (path: string, tool: string) => invoke<void>("launch_tool", { path, tool }),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<void>("save_settings", { settings }),
  detectTools: () => invoke<DetectedTools>("detect_tools"),
  aiToolsInfo: () => invoke<AiToolsInfo>("ai_tools_info"),
  aiClients: () => invoke<ClientStatus[]>("ai_clients"),
  connectClient: (client: string) => invoke<string>("connect_client", { client }),
  disconnectClient: (client: string) => invoke<string>("disconnect_client", { client }),
  listSessions: (path: string) => invoke<SessionInfo[]>("list_sessions", { path }),
  resumeSession: (path: string, id: string) => invoke<void>("resume_session", { path, id }),
  deleteSession: (path: string, id: string) => invoke<void>("delete_session", { path, id }),
};

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}
