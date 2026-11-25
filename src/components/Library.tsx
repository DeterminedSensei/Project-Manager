import type { ProjectSummary } from "../types";
import Cover from "./Cover";
import { formatDate } from "./format";

const ENGINE_LABEL: Record<string, string> = {
  unity: "Unity",
  unreal: "Unreal",
  godot: "Godot",
};

export default function Library({
  projects,
  onOpen,
  onAdd,
}: {
  projects: ProjectSummary[];
  onOpen: (path: string) => void;
  onAdd: () => void;
}) {
  if (projects.length === 0) {
    return (
      <div className="empty">
        <h2>Your library is empty</h2>
        <p className="muted">
          Add a project folder. Project Manager creates a <code>.projectmanager</code> folder in it
          for docs, the work log and the cover image.
        </p>
        <button className="btn primary" onClick={onAdd}>+ Add project</button>
      </div>
    );
  }

  return (
    <div className="library">
      <h2 className="section-title">
        All projects <span className="muted">({projects.length})</span>
      </h2>
      <div className="grid">
        {projects.map((p) => (
          <div key={p.path} className={"card" + (p.missing ? " missing" : "")} onClick={() => onOpen(p.path)}>
            <div className="card-cover">
              <Cover project={p} />
              {ENGINE_LABEL[p.meta.engine] && <span className="badge">{ENGINE_LABEL[p.meta.engine]}</span>}
            </div>
            <div className="card-info">
              <div className="card-title">{p.meta.name}</div>
              <div className="card-sub muted">
                {p.missing
                  ? "Folder not found"
                  : p.lastEntry
                    ? `${formatDate(p.lastEntry.date)} · ${p.lastEntry.author}`
                    : "No activity yet"}
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
