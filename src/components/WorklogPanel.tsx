import { useState, type ChangeEvent } from "react";
import { api, errorText } from "../api";
import { ENTRY_KINDS, type NewWorklogEntry, type WorklogEntry } from "../types";
import Markdown from "./Markdown";
import { formatDate } from "./format";

interface Props {
  path: string;
  entries: WorklogEntry[];
  notify: (text: string, error?: boolean) => void;
  onChanged: () => void;
}

const AUTHOR_KEY = "pm.author";

function emptyEntry(): NewWorklogEntry {
  let author = "";
  try {
    author = localStorage.getItem(AUTHOR_KEY) ?? "";
  } catch {
    /* storage unavailable */
  }
  return { author, kind: "feature", title: "", summary: "", changes: "", decisions: "", issues: "", nextSteps: "" };
}

export default function WorklogPanel({ path, entries, notify, onChanged }: Props) {
  const [form, setForm] = useState<NewWorklogEntry | null>(null);
  const [expanded, setExpanded] = useState<string | null>(entries[0]?.file ?? null);
  const [filter, setFilter] = useState("all");

  async function submit() {
    if (!form) return;
    if (!form.title.trim() || !form.summary.trim()) {
      notify("Title and summary are required", true);
      return;
    }
    try {
      localStorage.setItem(AUTHOR_KEY, form.author);
    } catch {
      /* ignore */
    }
    try {
      const e = await api.addWorklogEntry(path, form);
      setForm(null);
      setExpanded(e.file);
      onChanged();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  const set = (k: keyof NewWorklogEntry) => (e: ChangeEvent<HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement>) =>
    form && setForm({ ...form, [k]: e.target.value });

  const shown = filter === "all" ? entries : entries.filter((e) => e.kind === filter);

  return (
    <div className="worklog">
      <div className="worklog-bar">
        <button className="btn primary small" onClick={() => setForm(form ? null : emptyEntry())}>
          {form ? "Close form" : "+ New entry"}
        </button>
        <select value={filter} onChange={(e) => setFilter(e.target.value)}>
          <option value="all">All types</option>
          {ENTRY_KINDS.map((k) => (
            <option key={k} value={k}>{k}</option>
          ))}
        </select>
        <span className="muted small">Stored as Markdown files in .projectmanager/worklog/</span>
      </div>

      {form && (
        <div className="panel entry-form">
          <div className="form-row">
            <div>
              <label>Author</label>
              <input value={form.author} onChange={set("author")} placeholder="Your name or AI model" />
            </div>
            <div>
              <label>Type</label>
              <select value={form.kind} onChange={set("kind")}>
                {ENTRY_KINDS.map((k) => (
                  <option key={k} value={k}>{k}</option>
                ))}
              </select>
            </div>
          </div>
          <label>Title *</label>
          <input value={form.title} onChange={set("title")} placeholder="One-line summary" />
          <label>Summary *</label>
          <textarea rows={3} value={form.summary} onChange={set("summary")} placeholder="What was done and why" />
          <div className="form-row">
            <div>
              <label>Changes</label>
              <textarea rows={3} value={form.changes} onChange={set("changes")} placeholder="- `file` - what changed" />
            </div>
            <div>
              <label>Decisions</label>
              <textarea rows={3} value={form.decisions} onChange={set("decisions")} placeholder="- decision and reason" />
            </div>
          </div>
          <div className="form-row">
            <div>
              <label>Known issues</label>
              <textarea rows={3} value={form.issues} onChange={set("issues")} />
            </div>
            <div>
              <label>Next steps</label>
              <textarea rows={3} value={form.nextSteps} onChange={set("nextSteps")} />
            </div>
          </div>
          <div className="modal-actions">
            <button className="btn primary" onClick={submit}>Save entry</button>
          </div>
        </div>
      )}

      <div className="timeline">
        {shown.map((e) => (
          <div key={e.file} className={"entry" + (expanded === e.file ? " open" : "")}>
            <div className="entry-head" onClick={() => setExpanded(expanded === e.file ? null : e.file)}>
              <span className={"kind kind-" + e.kind}>{e.kind}</span>
              <span className="entry-title">{e.title}</span>
              <span className="muted small entry-meta">
                {e.author} · {formatDate(e.date)}
                {e.dateCorrected && (
                  <span
                    className="time-fixed"
                    title={`The entry says "${e.claimedDate}", which is later than its file was written. Showing the file's real time instead.`}
                  >
                    ⏱
                  </span>
                )}
              </span>
            </div>
            {expanded === e.file && (
              <div className="entry-body">
                <Markdown text={e.body} />
                <div className="muted small">worklog/{e.file}</div>
              </div>
            )}
          </div>
        ))}
        {shown.length === 0 && <div className="muted">No entries.</div>}
      </div>
    </div>
  );
}
