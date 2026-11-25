import { useCallback, useEffect, useState } from "react";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import type { CustomTab, ProjectDetail, ProjectMeta } from "../types";
import Cover from "./Cover";
import Markdown from "./Markdown";
import DocsPanel from "./DocsPanel";
import WorklogPanel from "./WorklogPanel";
import AiPanel from "./AiPanel";
import SessionsPanel from "./SessionsPanel";
import AddTabDialog from "./AddTabDialog";
import TabSettingsDialog from "./TabSettingsDialog";
import { formatDate } from "./format";

/** Built-in tab ids, or "tab:<id>" for an extra tab. */
type Tab = string;

const LAUNCH_ICON: Record<string, string> = {
  vscode: "⌨",
  intellij: "◧",
  unity: "◈",
  unreal: "▲",
  godot: "◉",
  terminal: "›_",
  explorer: "📁",
};

interface Props {
  path: string;
  notify: (text: string, error?: boolean) => void;
  onChanged: () => void;
  onRemoved: () => void;
}

export default function ProjectView({ path, notify, onChanged, onRemoved }: Props) {
  const [detail, setDetail] = useState<ProjectDetail | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [editingState, setEditingState] = useState<string | null>(null);
  const [editingMeta, setEditingMeta] = useState<ProjectMeta | null>(null);
  const [tagsText, setTagsText] = useState("");
  const [addingTab, setAddingTab] = useState(false);
  const [tabSettings, setTabSettings] = useState<CustomTab | null>(null);

  const load = useCallback(async () => {
    try {
      setDetail(await api.getProject(path));
      setLoadError(null);
    } catch (e) {
      setLoadError(errorText(e));
    }
  }, [path]);

  useEffect(() => {
    load();
  }, [load]);

  if (loadError) {
    return (
      <div className="empty">
        <h2>Could not open project</h2>
        <p className="muted">{loadError}</p>
      </div>
    );
  }
  if (!detail) return <div className="empty muted">Loading…</div>;

  const { summary } = detail;
  const meta = summary.meta;
  const claude = detail.launchers.find((l) => l.id === "claude");
  const customTabs = meta.tabs ?? [];
  const activeCustom = tab.startsWith("tab:") ? customTabs.find((t) => `tab:${t.id}` === tab) ?? null : null;
  const continueOpt = detail.launchers.find((l) => l.id === "claude-continue");
  const otherLaunchers = detail.launchers.filter((l) => l.id !== "claude" && l.id !== "claude-continue");

  async function launch(tool: string) {
    try {
      await api.launchTool(path, tool);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function changeCover() {
    const file = await open({
      multiple: false,
      title: "Choose a cover image",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "gif"] }],
    });
    if (typeof file !== "string") return;
    try {
      await api.setCover(path, file);
      await load();
      onChanged();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function saveMeta() {
    if (!editingMeta) return;
    const updated: ProjectMeta = {
      ...editingMeta,
      name: editingMeta.name.trim() || meta.name,
      tags: tagsText.split(",").map((t) => t.trim()).filter(Boolean),
    };
    try {
      await api.updateMeta(path, updated);
      setEditingMeta(null);
      await load();
      onChanged();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function saveState() {
    if (editingState === null) return;
    try {
      await api.saveState(path, editingState);
      setEditingState(null);
      await load();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function remove() {
    const ok = await ask(`Remove "${meta.name}" from the library?\n\nNo files are deleted.`, {
      title: "Remove project",
      kind: "warning",
    });
    if (!ok) return;
    try {
      await api.removeProject(path);
      onRemoved();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  if (summary.missing) {
    return (
      <div className="empty">
        <h2>{meta.name}</h2>
        <p className="muted">The folder <code>{path}</code> was not found, or it has no .projectmanager folder.</p>
        <button className="btn" onClick={remove}>Remove from library</button>
      </div>
    );
  }

  return (
    <div className="project">
      <section className="hero">
        {summary.coverUrl && <div className="hero-bg" style={{ backgroundImage: `url(${summary.coverUrl})` }} />}
        <div className="hero-content">
          <div className="hero-cover" onClick={changeCover} title="Change cover image">
            <Cover project={summary} />
            <div className="hero-cover-hint">Change cover</div>
          </div>
          <div className="hero-text">
            <h1>{meta.name}</h1>
            {meta.description && <p className="hero-desc">{meta.description}</p>}
            <div className="tags">
              {meta.engine !== "none" && <span className="tag engine">{meta.engine}</span>}
              {meta.tags.map((t) => (
                <span key={t} className="tag">{t}</span>
              ))}
            </div>
            <div className="path muted" title={path}>{path}</div>
            {claude && (
              <div className="claude-row">
                <button
                  className="btn claude-btn"
                  disabled={!claude.available}
                  title={claude.hint}
                  onClick={() => launch("claude")}
                >
                  ✳ Start Claude
                </button>
                {continueOpt && (
                  <button className="btn continue-btn" title={continueOpt.hint} onClick={() => launch("claude-continue")}>
                    ↻ Continue last session
                  </button>
                )}
                {!claude.available && <span className="muted small">{claude.hint}</span>}
              </div>
            )}
            <div className="launchers">
              {otherLaunchers.map((l, i) => (
                <button
                  key={l.id}
                  className={"btn launch" + (i === 0 && l.available ? " primary" : "")}
                  disabled={!l.available}
                  title={l.hint}
                  onClick={() => launch(l.id)}
                >
                  {l.icon ? (
                    <img className="launch-icon launch-img" src={l.icon} alt="" draggable={false} />
                  ) : (
                    <span className="launch-icon">{LAUNCH_ICON[l.id] ?? "▶"}</span>
                  )}{" "}
                  {l.label}
                </button>
              ))}
            </div>
          </div>
          <div className="hero-actions">
            <button
              className="btn ghost small"
              onClick={() => {
                setEditingMeta({ ...meta });
                setTagsText(meta.tags.join(", "));
              }}
            >
              Edit details
            </button>
            <button className="btn ghost small" onClick={remove}>Remove</button>
          </div>
        </div>
      </section>

      <nav className="tabs">
        {(
          [
            ["overview", "Overview"],
            ["docs", `Docs (${detail.docs.length})`],
            ...customTabs.map((t) => [`tab:${t.id}`, `${t.title} (${detail.tabPages[t.id]?.length ?? 0})`]),
            ["worklog", `Work Log (${detail.worklog.length})`],
            ["sessions", "Claude Sessions"],
            ["ai", "AI Access"],
          ] as [Tab, string][]
        ).map(([id, label]) => (
          <button key={id} className={"tab" + (tab === id ? " active" : "")} onClick={() => setTab(id)}>
            {label}
          </button>
        ))}
        <button className="tab add-tab" title="Add a tab, e.g. a Tutorial" onClick={() => setAddingTab(true)}>
          + Add tab
        </button>
        <div className="tabs-spacer" />
        <button className="btn ghost small" onClick={load} title="Reload files from disk">⟳ Reload</button>
      </nav>

      <section className="tab-body">
        {tab === "overview" && (
          <div className="overview">
            <div className="panel state-panel">
              <div className="panel-head">
                <h3>Current state <span className="muted small">STATE.md</span></h3>
                {editingState === null ? (
                  <button className="btn small" onClick={() => setEditingState(detail.state)}>Edit</button>
                ) : (
                  <div className="row">
                    <button className="btn small" onClick={() => setEditingState(null)}>Cancel</button>
                    <button className="btn small primary" onClick={saveState}>Save</button>
                  </div>
                )}
              </div>
              {editingState === null ? (
                <Markdown text={detail.state || "_No STATE.md yet._"} />
              ) : (
                <textarea
                  className="editor"
                  value={editingState}
                  onChange={(e) => setEditingState(e.target.value)}
                  spellCheck={false}
                />
              )}
            </div>
            <div className="panel recent-panel">
              <div className="panel-head">
                <h3>Recent activity</h3>
                <button className="btn small" onClick={() => setTab("worklog")}>Open log</button>
              </div>
              {detail.worklog.slice(0, 6).map((e) => (
                <div key={e.file} className="recent-item">
                  <span className={"kind kind-" + e.kind}>{e.kind}</span>
                  <div>
                    <div className="recent-title">{e.title}</div>
                    <div className="muted small">{formatDate(e.date)} · {e.author}</div>
                  </div>
                </div>
              ))}
              {detail.worklog.length === 0 && <div className="muted">No entries yet.</div>}
            </div>
          </div>
        )}
        {tab === "docs" && (
          <DocsPanel
            key="docs"
            path={path}
            section="docs"
            pages={detail.docs}
            notify={notify}
            onChanged={load}
            emptyText="No documentation pages yet. Create one on the left."
          />
        )}
        {activeCustom && (
          <DocsPanel
            key={tab}
            path={path}
            section={activeCustom.id}
            pages={detail.tabPages[activeCustom.id] ?? []}
            notify={notify}
            onChanged={load}
            emptyText="No pages yet. AI agents working on this project will add them, or create one on the left."
            sidebarExtra={
              <div className="tab-info muted">
                AI agents keep this tab up to date automatically. Stored in{" "}
                <code>.projectmanager/{activeCustom.id}/</code>.
                <button className="btn small" onClick={() => setTabSettings(activeCustom)}>Tab settings</button>
              </div>
            }
          />
        )}
        {tab === "worklog" && (
          <WorklogPanel path={path} entries={detail.worklog} notify={notify} onChanged={() => { load(); onChanged(); }} />
        )}
        {tab === "sessions" && (
          <SessionsPanel path={path} claudeAvailable={!!claude?.available} notify={notify} onChanged={load} />
        )}
        {tab === "ai" && (
          <AiPanel
            path={path}
            name={meta.name}
            aiFolders={detail.aiFolders}
            pointerTarget={detail.pointerTarget}
            notify={notify}
            onChanged={load}
          />
        )}
      </section>

      {addingTab && (
        <AddTabDialog
          path={path}
          engine={meta.engine}
          existing={customTabs}
          notify={notify}
          onClose={() => setAddingTab(false)}
          onAdded={async (t) => {
            setAddingTab(false);
            await load();
            setTab(`tab:${t.id}`);
          }}
        />
      )}
      {tabSettings && (
        <TabSettingsDialog
          path={path}
          tab={tabSettings}
          notify={notify}
          onClose={() => setTabSettings(null)}
          onChanged={async (removed) => {
            setTabSettings(null);
            if (removed) setTab("overview");
            await load();
          }}
        />
      )}
      {editingMeta && (
        <div className="modal-backdrop" onClick={() => setEditingMeta(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h3>Edit details</h3>
            <label>Name</label>
            <input value={editingMeta.name} onChange={(e) => setEditingMeta({ ...editingMeta, name: e.target.value })} />
            <label>Description</label>
            <textarea
              rows={3}
              value={editingMeta.description}
              onChange={(e) => setEditingMeta({ ...editingMeta, description: e.target.value })}
            />
            <label>Engine</label>
            <select value={editingMeta.engine} onChange={(e) => setEditingMeta({ ...editingMeta, engine: e.target.value })}>
              <option value="none">None / other</option>
              <option value="unity">Unity</option>
              <option value="unreal">Unreal</option>
              <option value="godot">Godot</option>
            </select>
            <label>Tags (comma-separated)</label>
            <input value={tagsText} onChange={(e) => setTagsText(e.target.value)} placeholder="game, 2D, prototype" />
            <div className="modal-actions">
              <button className="btn" onClick={() => setEditingMeta(null)}>Cancel</button>
              <button className="btn primary" onClick={saveMeta}>Save</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
