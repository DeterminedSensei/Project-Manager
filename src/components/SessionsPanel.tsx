import { useCallback, useEffect, useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import type { SessionInfo } from "../types";
import { formatDate } from "./format";

interface Props {
  path: string;
  claudeAvailable: boolean;
  notify: (text: string, error?: boolean) => void;
  onChanged: () => void;
}

function sizeText(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export default function SessionsPanel({ path, claudeAvailable, notify, onChanged }: Props) {
  const [sessions, setSessions] = useState<SessionInfo[] | null>(null);

  const load = useCallback(async () => {
    try {
      setSessions(await api.listSessions(path));
    } catch (e) {
      notify(errorText(e), true);
      setSessions([]);
    }
  }, [path, notify]);

  useEffect(() => {
    load();
  }, [load]);

  async function resume(s: SessionInfo) {
    try {
      await api.resumeSession(path, s.id);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function remove(s: SessionInfo) {
    const title = s.name ?? s.summary ?? s.firstPrompt ?? s.id;
    const ok = await ask(
      `Delete this Claude Code session permanently?\n\n"${title}"\n\nThe conversation can't be restored afterwards. Files in your project are not affected.`,
      { title: "Delete session", kind: "warning" },
    );
    if (!ok) return;
    try {
      await api.deleteSession(path, s.id);
      notify("Session deleted");
      await load();
      onChanged();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  return (
    <div className="sessions">
      <div className="worklog-bar">
        <span className="muted small">
          Claude Code (terminal) sessions started in this folder, newest first. Chats in the Claude app are stored in
          your account and don't appear here.
        </span>
        <div className="tabs-spacer" />
        <button className="btn ghost small" onClick={load}>⟳ Refresh</button>
      </div>

      {sessions === null && <div className="muted">Loading…</div>}
      {sessions !== null && sessions.length === 0 && (
        <div className="panel muted">No Claude Code sessions for this folder yet. Click Start Claude to begin one.</div>
      )}

      <div className="timeline">
        {sessions?.map((s) => {
          const title = s.name ?? s.summary ?? s.firstPrompt ?? s.id;
          return (
            <div key={s.id} className="entry session">
              <div className="session-main">
                <div className="session-title">{title}</div>
                {s.firstPrompt && s.firstPrompt !== title && <div className="muted small session-prompt">“{s.firstPrompt}”</div>}
                <div className="muted small">
                  {formatDate(s.lastActivity ?? s.started ?? "")}
                  {" · "}
                  {s.messageCount} messages
                  {s.gitBranch ? ` · ${s.gitBranch}` : ""}
                  {" · "}
                  {sizeText(s.sizeBytes)}
                </div>
              </div>
              <div className="session-actions">
                <button
                  className="btn small primary"
                  disabled={!claudeAvailable}
                  title={claudeAvailable ? "Open a terminal here and resume this session" : "Claude Code was not found"}
                  onClick={() => resume(s)}
                >
                  Resume
                </button>
                <button className="btn small ghost danger" title="Delete this session" onClick={() => remove(s)}>
                  Delete
                </button>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
