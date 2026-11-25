import { useEffect, useState } from "react";
import { api, errorText } from "../api";
import type { CustomTab, TabPreset } from "../types";

interface Props {
  path: string;
  engine: string;
  existing: CustomTab[];
  onClose: () => void;
  onAdded: (tab: CustomTab) => void;
  notify: (text: string, error?: boolean) => void;
}

export default function AddTabDialog({ path, engine, existing, onClose, onAdded, notify }: Props) {
  const [presets, setPresets] = useState<TabPreset[]>([]);
  const [preset, setPreset] = useState<string>("");
  const [title, setTitle] = useState("");
  const [instructions, setInstructions] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .tabPresets()
      .then((list) => {
        setPresets(list);
        // Suggest the first preset the project doesn't have yet (Tutorial for games).
        const taken = new Set(existing.map((t) => t.preset));
        const first = list.find((p) => p.id !== "custom" && !taken.has(p.id)) ?? list[list.length - 1];
        if (first) choose(first);
      })
      .catch((e) => notify(errorText(e), true));
  }, []);

  function choose(p: TabPreset) {
    setPreset(p.id);
    setTitle(p.title);
    setInstructions(p.instructions);
  }

  async function add() {
    if (!title.trim()) {
      notify("Give the tab a name", true);
      return;
    }
    setBusy(true);
    try {
      const tab = await api.addTab(path, title, preset || "custom", instructions);
      notify(`Added the "${tab.title}" tab`);
      onAdded(tab);
    } catch (e) {
      notify(errorText(e), true);
    } finally {
      setBusy(false);
    }
  }

  const isGame = engine !== "none";

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h3>Add a tab</h3>
        <p className="muted small">
          The tab's pages are stored in <code>.projectmanager/&lt;tab&gt;/</code>. Its instructions are added to the
          project's <code>AGENTS.md</code>, so every AI agent working on the project keeps the tab up to date
          automatically.
        </p>
        <div className="preset-grid">
          {presets.map((p) => (
            <button
              key={p.id}
              className={"preset" + (preset === p.id ? " active" : "")}
              onClick={() => choose(p)}
            >
              <strong>{p.id === "custom" ? "Custom" : p.title}</strong>
              {p.id === "tutorial" && isGame && <span className="preset-tag">for games</span>}
              <span className="muted small">{p.description}</span>
            </button>
          ))}
        </div>
        <label>Tab name</label>
        <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="e.g. Tutorial, Lore, Balancing" />
        <label>Instructions for AI agents</label>
        <textarea
          rows={8}
          value={instructions}
          onChange={(e) => setInstructions(e.target.value)}
          placeholder="What should AI agents keep in this tab, and when should they update it?"
          spellCheck={false}
        />
        <div className="modal-actions">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" disabled={busy} onClick={add}>
            {busy ? "Adding…" : "Add tab"}
          </button>
        </div>
      </div>
    </div>
  );
}
