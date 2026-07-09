import { useState } from "react";
import { useWorkspace } from "../hooks/useWorkspace";

type Kind = "file" | "web" | "youtube";

const FIELDS: Array<{ kind: Kind; label: string; placeholder: string }> = [
  { kind: "file", label: "Local file", placeholder: "/path/to/notes.pdf" },
  { kind: "web", label: "Web page", placeholder: "https://…" },
  { kind: "youtube", label: "YouTube", placeholder: "https://youtu.be/…" },
];

/** Ingest sources by path/URL. (A native file picker is a future enhancement.) */
export function UploadZone() {
  const { ingest } = useWorkspace();
  const [values, setValues] = useState<Record<Kind, string>>({ file: "", web: "", youtube: "" });
  const [busyKind, setBusyKind] = useState<Kind | null>(null);
  const [error, setError] = useState<string | null>(null);

  const submit = async (kind: Kind): Promise<void> => {
    const value = values[kind];
    if (!value.trim() || busyKind) return;
    setBusyKind(kind);
    setError(null);
    try {
      await ingest(kind, value);
      setValues((v) => ({ ...v, [kind]: "" }));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusyKind(null);
    }
  };

  return (
    <div className="space-y-2">
      {FIELDS.map(({ kind, label, placeholder }) => (
        <div key={kind}>
          <label className="mb-1 block text-[11px] uppercase tracking-wide text-neutral-500">
            {label}
          </label>
          <div className="flex gap-1">
            <input
              value={values[kind]}
              onChange={(e) => setValues((v) => ({ ...v, [kind]: e.target.value }))}
              onKeyDown={(e) => {
                if (e.key === "Enter") void submit(kind);
              }}
              placeholder={placeholder}
              disabled={busyKind !== null}
              className="min-w-0 flex-1 rounded-md border border-neutral-700 bg-neutral-900 px-2 py-1 text-xs text-neutral-100 placeholder:text-neutral-600 focus:border-blue-500 focus:outline-none disabled:opacity-50"
            />
            <button
              onClick={() => void submit(kind)}
              disabled={busyKind !== null || !values[kind].trim()}
              className="rounded-md bg-neutral-800 px-2 py-1 text-xs text-neutral-200 hover:bg-neutral-700 disabled:opacity-40"
            >
              {busyKind === kind ? "…" : "Add"}
            </button>
          </div>
        </div>
      ))}
      {error && <p className="text-[11px] text-red-400">{error}</p>}
    </div>
  );
}
