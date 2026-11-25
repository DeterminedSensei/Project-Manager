import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { api, errorText } from "../api";
import type { DetectedTools, Settings } from "../types";

interface Props {
  onClose: () => void;
  notify: (text: string, error?: boolean) => void;
}

const FIELDS: { key: keyof Settings; label: string; folder: boolean; help: string }[] = [
  { key: "vscode", label: "VS Code", folder: false, help: "Code.exe" },
  { key: "intellij", label: "IntelliJ IDEA", folder: false, help: "bin\\idea64.exe" },
  {
    key: "unityEditorsDir",
    label: "Unity editors folder",
    folder: true,
    help: "Unity Hub's Editor folder, which has one sub-folder per version",
  },
  { key: "unrealEditor", label: "Unreal Editor", folder: false, help: "UnrealEditor.exe (optional: without it, .uproject opens with its file association)" },
  { key: "godot", label: "Godot", folder: false, help: "Godot_v4.x.exe" },
];

const EMPTY: Settings = { vscode: "", intellij: "", unityEditorsDir: "", unrealEditor: "", godot: "" };

export default function SettingsDialog({ onClose, notify }: Props) {
  const [settings, setSettings] = useState<Settings>(EMPTY);
  const [detected, setDetected] = useState<DetectedTools>({});

  useEffect(() => {
    api.getSettings().then((s) => setSettings({ ...EMPTY, ...s })).catch((e) => notify(errorText(e), true));
    api.detectTools().then(setDetected).catch(() => {});
  }, [notify]);

  async function browse(key: keyof Settings, folder: boolean) {
    const picked = await open({
      directory: folder,
      multiple: false,
      filters: folder ? undefined : [{ name: "Programs", extensions: ["exe"] }],
    });
    if (typeof picked === "string") setSettings({ ...settings, [key]: picked });
  }

  async function save() {
    try {
      await api.saveSettings(settings);
      notify("Settings saved");
      onClose();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal wide" onClick={(e) => e.stopPropagation()}>
        <h3>Settings: editor paths</h3>
        <p className="muted small">Leave a field empty to auto-detect. Detected paths are shown below each field.</p>
        {FIELDS.map((f) => (
          <div key={f.key} className="setting">
            <label>{f.label}</label>
            <div className="row">
              <input
                value={settings[f.key]}
                placeholder={f.help}
                onChange={(e) => setSettings({ ...settings, [f.key]: e.target.value })}
              />
              <button className="btn small" onClick={() => browse(f.key, f.folder)}>Browse…</button>
            </div>
            <div className="muted small">
              {detected[f.key] ? `Using: ${detected[f.key]}` : "Not found"}
            </div>
          </div>
        ))}
        <div className="modal-actions">
          <button className="btn" onClick={onClose}>Cancel</button>
          <button className="btn primary" onClick={save}>Save</button>
        </div>
      </div>
    </div>
  );
}
