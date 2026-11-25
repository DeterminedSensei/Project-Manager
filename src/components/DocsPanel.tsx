import { useEffect, useState, type ReactNode } from "react";
import { api, errorText } from "../api";
import Markdown from "./Markdown";

interface Props {
  path: string;
  /** "docs" or the id of an extra tab such as "tutorial". */
  section: string;
  pages: string[];
  notify: (text: string, error?: boolean) => void;
  onChanged: () => void;
  /** Shown above the page list, e.g. a tab settings button. */
  sidebarExtra?: ReactNode;
  emptyText?: string;
}

function docTitle(name: string, content?: string): string {
  const h1 = content?.match(/^#\s+(.+)$/m)?.[1];
  if (h1) return h1.trim();
  return name
    .replace(/\.md$/i, "")
    .replace(/^\d+[-_]/, "")
    .replace(/[-_]+/g, " ");
}

export default function DocsPanel({ path, section, pages, notify, onChanged, sidebarExtra, emptyText }: Props) {
  const docs = pages;
  const [current, setCurrent] = useState<string | null>(docs[0] ?? null);
  const [content, setContent] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState<string | null>(null);

  useEffect(() => {
    if (!current) {
      setContent("");
      return;
    }
    setDraft(null);
    api
      .readPage(path, section, current)
      .then(setContent)
      .catch((e) => notify(errorText(e), true));
  }, [path, section, current, notify]);

  async function save() {
    if (!current || draft === null) return;
    try {
      await api.savePage(path, section, current, draft);
      setContent(draft);
      setDraft(null);
      onChanged();
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  async function create() {
    const title = newTitle?.trim();
    if (!title) return;
    try {
      const name = await api.createPage(path, section, title);
      setNewTitle(null);
      onChanged();
      setCurrent(name);
    } catch (e) {
      notify(errorText(e), true);
    }
  }

  return (
    <div className="docs">
      <aside className="docs-nav">
        {sidebarExtra}
        {docs.map((d) => (
          <div key={d} className={"docs-item" + (d === current ? " active" : "")} onClick={() => setCurrent(d)}>
            {docTitle(d, d === current ? content : undefined)}
          </div>
        ))}
        {newTitle === null ? (
          <button className="btn small block" onClick={() => setNewTitle("")}>+ New page</button>
        ) : (
          <div className="new-doc">
            <input
              autoFocus
              placeholder="Page title"
              value={newTitle}
              onChange={(e) => setNewTitle(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") create();
                if (e.key === "Escape") setNewTitle(null);
              }}
            />
            <div className="row">
              <button className="btn small" onClick={() => setNewTitle(null)}>Cancel</button>
              <button className="btn small primary" onClick={create}>Create</button>
            </div>
          </div>
        )}
      </aside>
      <div className="panel docs-content">
        {current ? (
          <>
            <div className="panel-head">
              <span className="muted small">{section}/{current}</span>
              {draft === null ? (
                <button className="btn small" onClick={() => setDraft(content)}>Edit</button>
              ) : (
                <div className="row">
                  <button className="btn small" onClick={() => setDraft(null)}>Cancel</button>
                  <button className="btn small primary" onClick={save}>Save</button>
                </div>
              )}
            </div>
            {draft === null ? (
              <Markdown text={content} />
            ) : (
              <textarea className="editor tall" value={draft} onChange={(e) => setDraft(e.target.value)} spellCheck={false} />
            )}
          </>
        ) : (
          <div className="muted">{emptyText ?? "No pages yet. Create one on the left."}</div>
        )}
      </div>
    </div>
  );
}
