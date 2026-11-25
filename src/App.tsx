import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "./api";
import type { ProjectSummary } from "./types";
import Library from "./components/Library";
import ProjectView from "./components/ProjectView";
import SettingsDialog from "./components/SettingsDialog";
import Cover from "./components/Cover";
import PointerChooser, { defaultPointer } from "./components/PointerChooser";

interface PointerSetup {
  path: string;
  name: string;
  aiFolders: string[];
  target: string;
}

export default function App() {
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [toast, setToast] = useState<{ text: string; error: boolean } | null>(null);
  const [setup, setSetup] = useState<PointerSetup | null>(null);
  const [reloadKey, setReloadKey] = useState(0);

  const notify = useCallback((text: string, error = false) => {
    setToast({ text, error });
    window.setTimeout(() => setToast(null), error ? 6000 : 3000);
  }, []);

  const refresh = useCallback(async () => {
    try {
      setProjects(await api.listProjects());
    } catch (e) {
      notify(errorText(e), true);
    }
  }, [notify]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  async function addProject() {
    const folder = await open({ directory: true, multiple: false, title: "Choose a project folder" });
    if (typeof folder !== "string") return;
    try {
      const p = await api.addProject(folder);
      await refresh();
      setSelected(p.path);
      notify(`Added ${p.meta.name}`);
      // Offer to point AGENTS.md / CLAUDE.md to the rules, unless that was done before.
      const d = await api.getProject(p.path);
      if (!d.pointerTarget) {
        setSetup({ path: p.path, name: p.meta.name, aiFolders: d.aiFolders, target: defaultPointer(d.aiFolders) });
      }
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function applySetup() {
    if (!setup) return;
    try {
      const changed = await api.installAiPointer(setup.path, setup.target);
      notify(changed.length ? `Updated: ${changed.join(", ")}` : "Already up to date");
      setSetup(null);
      setReloadKey((k) => k + 1);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  const filtered = projects.filter((p) => {
    const q = search.trim().toLowerCase();
    if (!q) return true;
    return (
      p.meta.name.toLowerCase().includes(q) ||
      p.meta.tags.some((t) => t.toLowerCase().includes(q)) ||
      p.meta.engine.toLowerCase().includes(q)
    );
  });

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand" onClick={() => setSelected(null)}>
          <span className="brand-mark">◆</span> PROJECT MANAGER
        </div>
        <nav className="topnav">
          <button className={selected === null ? "nav active" : "nav"} onClick={() => setSelected(null)}>
            Library
          </button>
        </nav>
        <div className="topbar-right">
          <button className="btn primary" onClick={addProject}>+ Add project</button>
          <button className="btn ghost" title="Settings" onClick={() => setShowSettings(true)}>⚙</button>
        </div>
      </header>

      <div className="body">
        <aside className="sidebar">
          <input
            className="search"
            placeholder="Search projects…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
          <div className="sidebar-list">
            {filtered.map((p) => (
              <div
                key={p.path}
                className={"sidebar-item" + (selected === p.path ? " active" : "") + (p.missing ? " missing" : "")}
                onClick={() => setSelected(p.path)}
                title={p.path}
              >
                <Cover project={p} small />
                <span className="sidebar-name">{p.meta.name}</span>
              </div>
            ))}
            {projects.length === 0 && <div className="muted pad">No projects yet.</div>}
          </div>
        </aside>

        <main className="main">
          {selected === null ? (
            <Library projects={filtered} onOpen={setSelected} onAdd={addProject} />
          ) : (
            <ProjectView
              key={`${selected}#${reloadKey}`}
              path={selected}
              notify={notify}
              onChanged={refresh}
              onRemoved={() => {
                setSelected(null);
                refresh();
              }}
            />
          )}
        </main>
      </div>

      {setup && (
        <div className="modal-backdrop">
          <div className="modal wide">
            <h3>Let AI agents find the rules for {setup.name}?</h3>
            <p className="muted">
              {setup.aiFolders.length > 0 ? (
                <>
                  This project already has an AI folder ({setup.aiFolders.map((f) => <code key={f}>{f} </code>)}). Project
                  Manager can put its rules there and make <code>AGENTS.md</code> and <code>CLAUDE.md</code> in the
                  project root point to it.
                </>
              ) : (
                <>
                  Project Manager can add a short section to <code>AGENTS.md</code> and <code>CLAUDE.md</code> in the
                  project root, so coding agents read the logging rules automatically.
                </>
              )}{" "}
              Existing content in those files is kept.
            </p>
            <PointerChooser
              aiFolders={setup.aiFolders}
              value={setup.target}
              onChange={(target) => setSetup({ ...setup, target })}
            />
            <div className="modal-actions">
              <button className="btn" onClick={() => setSetup(null)}>Not now</button>
              <button className="btn primary" onClick={applySetup}>Add pointer</button>
            </div>
            <p className="muted small">You can change this later in the project's AI Access tab.</p>
          </div>
        </div>
      )}
      {showSettings && <SettingsDialog onClose={() => setShowSettings(false)} notify={notify} />}
      {toast && <div className={"toast" + (toast.error ? " error" : "")}>{toast.text}</div>}
    </div>
  );
}
