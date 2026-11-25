// Lets the user choose where the project's root AGENTS.md / CLAUDE.md should
// point AI agents: an existing AI folder (e.g. .claude) or .projectmanager.

export const PM_FOLDER = ".projectmanager";

export function pointerOptions(aiFolders: string[]): string[] {
  return [...aiFolders, PM_FOLDER];
}

export function defaultPointer(aiFolders: string[], current?: string | null): string {
  const options = pointerOptions(aiFolders);
  if (current && options.includes(current)) return current;
  return options[0];
}

export function pointerFile(target: string): string {
  return target === PM_FOLDER ? `${PM_FOLDER}/AGENTS.md` : `${target}/project-manager.md`;
}

export default function PointerChooser({
  aiFolders,
  value,
  onChange,
}: {
  aiFolders: string[];
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="pointer-options">
      {pointerOptions(aiFolders).map((t) => (
        <label key={t} className={"pointer-option" + (value === t ? " active" : "")}>
          <input type="radio" name="pointer-target" checked={value === t} onChange={() => onChange(t)} />
          <span>
            <code>{t}</code>
            {t === PM_FOLDER ? (
              <span className="muted small"> Project Manager's own folder</span>
            ) : (
              <span className="muted small"> existing AI folder, rules go to <code>{pointerFile(t)}</code></span>
            )}
          </span>
        </label>
      ))}
    </div>
  );
}
