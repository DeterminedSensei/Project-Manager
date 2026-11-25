import type { ProjectSummary } from "../types";

// Deterministic gradient + initials when a project has no cover image.
function hue(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 360;
  return h;
}

function initials(name: string): string {
  const words = name.replace(/[_\-.]+/g, " ").trim().split(/\s+/).filter(Boolean);
  if (words.length === 0) return "?";
  if (words.length === 1) return words[0].slice(0, 2).toUpperCase();
  return (words[0][0] + words[1][0]).toUpperCase();
}

export default function Cover({
  project,
  small = false,
  className = "",
}: {
  project: ProjectSummary;
  small?: boolean;
  className?: string;
}) {
  const cls = (small ? "cover cover-small " : "cover ") + className;
  if (project.coverUrl) {
    return <img className={cls} src={project.coverUrl} alt="" draggable={false} />;
  }
  const h = hue(project.meta.name);
  return (
    <div
      className={cls + " cover-placeholder"}
      style={{ background: `linear-gradient(135deg, hsl(${h} 55% 32%), hsl(${(h + 50) % 360} 60% 16%))` }}
    >
      <span>{initials(project.meta.name)}</span>
    </div>
  );
}
