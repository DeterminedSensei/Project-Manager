import { useState } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import type { CustomTab } from "../types";

interface Props {
  path: string;
  tab: CustomTab;
  onClose: () => void;
  onChanged: (removed: boolean) => void;
  notify: (text: string, error?: boolean) => void;
}

export default function TabSettingsDialog({ path, tab, onClose, onChanged, notify }: Props) {
  const [title, setTitle] = useState(tab.title);
  const [instructions, setInstructions] = useState(tab.instructions);

  async function save() {
    try {
      await api.updateTab(path, tab.id, title, instructions);
      notify("Tab updated");
      onChanged(false);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function remove() {
    const ok = await ask(
      `Remove the "${tab.title}" tab?\n\nAI agents will stop maintaining it. Its pages stay in .projectmanager/${tab.id}/ and come back if you add a tab with the same name again.`,
      { title: "Remove tab", kind: "warning" },
    );
    if (!ok) return;
    try {
      await api.removeTab(path, tab.id);
      notify(`Removed the "${tab.title}" tab`);
      onChanged(true);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h3>Tab settings</h3>
        <p className="muted small">
          Folder: <code>.projectmanager/{tab.id}/</code>
        </p>
        <label>Tab name</label>
        <input value={title} onChange={(e) => setTitle(e.target.value)} />
        <label>Instructions for AI agents (written into AGENTS.md)</label>
        <textarea rows={9} value={instructions} onChange={(e) => setInstructions(e.target.value)} spellCheck={false} />
        <div className="modal-actions">
          <button className="btn ghost danger" onClick={remove}>Remove tab</button>
          <div className="tabs-spacer" />
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={save}>Save</button>
        </div>
      </div>
    </div>
  );
}
