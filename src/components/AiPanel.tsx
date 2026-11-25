import { useEffect, useState } from "react";
import { api, errorText } from "../api";
import type { AiToolsInfo, ClientStatus } from "../types";
import PointerChooser, { defaultPointer, pointerFile } from "./PointerChooser";

interface Props {
  path: string;
  name: string;
  aiFolders: string[];
  pointerTarget?: string | null;
  notify: (text: string, error?: boolean) => void;
  onChanged: () => void;
}

export default function AiPanel({ path, name, aiFolders, pointerTarget, notify, onChanged }: Props) {
  const [target, setTarget] = useState(() => defaultPointer(aiFolders, pointerTarget));
  const [tools, setTools] = useState<AiToolsInfo | null>(null);
  const [clients, setClients] = useState<ClientStatus[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [messages, setMessages] = useState<Record<string, { text: string; error: boolean }>>({});

  async function refreshClients() {
    try {
      setClients(await api.aiClients());
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  useEffect(() => {
    api.aiToolsInfo().then(setTools).catch(() => setTools({ pmPath: null }));
    refreshClients();
  }, []);

  async function toggle(c: ClientStatus) {
    setBusy(c.id);
    try {
      const text = c.connected ? await api.disconnectClient(c.id) : await api.connectClient(c.id);
      setMessages((m) => ({ ...m, [c.id]: { text, error: false } }));
    } catch (e) {
      setMessages((m) => ({ ...m, [c.id]: { text: errorText(e), error: true } }));
    } finally {
      setBusy(null);
      refreshClients();
    }
  }

  const prompt =
    `You are working on the project "${name}" in the folder:\n${path}\n\n` +
    `Before doing anything else, read .projectmanager/AGENTS.md in that folder and follow it: ` +
    `read .projectmanager/STATE.md, the most recent files in .projectmanager/worklog/ and the relevant pages in .projectmanager/docs/. ` +
    `When you finish your work, add a new worklog entry and update STATE.md and the docs exactly as AGENTS.md describes.`;

  const pm = tools?.pmPath ?? null;
  const desktopConfig = pm
    ? JSON.stringify({ mcpServers: { "project-manager": { command: pm, args: ["mcp"] } } }, null, 2)
    : "";
  const claudeCodeCmd = pm ? `claude mcp add --scope user project-manager -- "${pm}" mcp` : "";

  async function copy(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text);
      notify(`${what} copied`);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function installPointer() {
    try {
      const changed = await api.installAiPointer(path, target);
      notify(changed.length ? `Updated: ${changed.join(", ")}` : "Already up to date");
      onChanged();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  return (
    <div className="ai">
      <div className="panel">
        <h3>How AI models work with this project</h3>
        <p>
          Everything lives as plain files in <code>.projectmanager/</code>, so any AI agent that can read and
          write files can use it, even when this app is closed:
        </p>
        <ul>
          <li><code>AGENTS.md</code>: the rules: what to read first and how to log work</li>
          <li><code>STATE.md</code>: a short snapshot of where the project stands</li>
          <li><code>worklog/</code>: one Markdown file per work session, append-only</li>
          <li><code>docs/</code>: documentation and tutorial pages</li>
        </ul>
      </div>

      <div className="panel">
        <h3>1. Connect AI assistants (MCP)</h3>
        <p className="muted">
          The <code>pm.exe</code> tool includes an MCP server. Once connected, assistants such as Claude Desktop,
          Claude Code, Cursor or VS Code can list your projects, read their context, write docs and add work log
          entries through tools. Only projects in this library are accessible. Click Connect once; the assistant then
          starts pm.exe by itself whenever it needs it.
        </p>
        {tools === null ? (
          <p className="muted">Looking for pm.exe…</p>
        ) : pm ? (
          <>
            <div className="clients">
              {clients.map((c) => (
                <div key={c.id} className="client">
                  <div className="client-head">
                    <span className={"dot" + (c.connected ? " on" : c.outdated ? " warn" : "")} />
                    <strong>{c.name}</strong>
                    <span className="client-state muted small">
                      {!c.installed ? "Not installed" : c.connected ? "Connected" : c.outdated ? "Needs update" : "Not connected"}
                    </span>
                    <button
                      className={"btn small" + (c.connected ? "" : " primary")}
                      disabled={!c.installed || busy !== null}
                      onClick={() => toggle(c)}
                    >
                      {busy === c.id ? "Working…" : c.connected ? "Disconnect" : c.outdated ? "Update" : "Connect"}
                    </button>
                  </div>
                  <div className="muted small">{c.detail}</div>
                  {messages[c.id] && (
                    <div className={"client-msg small" + (messages[c.id].error ? " error" : "")}>{messages[c.id].text}</div>
                  )}
                </div>
              ))}
            </div>
            <p className="small">
              <span className="muted">pm.exe:</span> <code>{pm}</code>
            </p>
            <details className="manual">
              <summary>Set up manually</summary>
              <h4>Claude Desktop</h4>
              <p className="muted small">
                Settings → Developer → Edit Config opens <code>claude_desktop_config.json</code>. Merge this in, then
                quit Claude Desktop from the tray icon and start it again:
              </p>
              <pre className="prompt">{desktopConfig}</pre>
              <button className="btn small" onClick={() => copy(desktopConfig, "Config")}>Copy config</button>
              <h4>Claude Code</h4>
              <p className="muted small">Run once in a terminal:</p>
              <pre className="prompt">{claudeCodeCmd}</pre>
              <button className="btn small" onClick={() => copy(claudeCodeCmd, "Command")}>Copy command</button>
              <h4>Other MCP clients</h4>
              <p className="muted small">
                Add a stdio server with command <code>{pm}</code> and argument <code>mcp</code>.
              </p>
            </details>
          </>
        ) : (
          <p className="muted">
            pm.exe has not been built yet. In the Project Manager source folder, run{" "}
            <code>npm run build:pm</code>, then reopen this tab.
          </p>
        )}
      </div>

      <div className="panel">
        <h3>2. Let agents find the rules automatically</h3>
        <p className="muted">
          Adds a short Project Manager section to <code>AGENTS.md</code> and <code>CLAUDE.md</code> in the project
          root (created if missing; an earlier section is replaced, the rest is kept). Coding agents read these files
          automatically. Choose where they should point:
        </p>
        <PointerChooser aiFolders={aiFolders} value={target} onChange={setTarget} />
        <p className="muted small">
          {pointerTarget
            ? <>Currently pointing to <code>{pointerTarget}</code>.</>
            : "No pointer added yet."}{" "}
          Agents will be told to read <code>{pointerFile(target)}</code>.
        </p>
        <button className="btn primary" onClick={installPointer}>
          {pointerTarget ? "Update pointer files" : "Add pointer files"}
        </button>
      </div>

      <div className="panel">
        <h3>3. Or brief an AI by hand</h3>
        <p className="muted">Paste this at the start of a chat with any AI model:</p>
        <pre className="prompt">{prompt}</pre>
        <button className="btn" onClick={() => copy(prompt, "Prompt")}>Copy prompt</button>
      </div>
    </div>
  );
}
