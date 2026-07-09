import { useWorkspace } from "../hooks/useWorkspace";

/** Display name for a source path or URL. */
function displayName(source: string): string {
  try {
    if (source.startsWith("http")) return new URL(source).hostname + new URL(source).pathname;
  } catch {
    /* fall through to path handling */
  }
  const parts = source.split("/");
  return parts[parts.length - 1] || source;
}

/** List of ingested sources with chunk counts. */
export function FileTree() {
  const { sources } = useWorkspace();

  if (sources.length === 0) {
    return (
      <p className="px-1 py-2 text-xs text-neutral-600">
        No documents yet. Add a file or link above to build your knowledge base.
      </p>
    );
  }

  return (
    <ul className="space-y-0.5">
      {sources.map((s) => (
        <li
          key={s.sourceFile}
          className="flex items-center justify-between gap-2 rounded px-2 py-1 text-xs text-neutral-300 hover:bg-neutral-800"
          title={s.sourceFile}
        >
          <span className="truncate">{displayName(s.sourceFile)}</span>
          <span className="shrink-0 rounded bg-neutral-800 px-1.5 text-[10px] text-neutral-500">
            {s.chunkCount}
          </span>
        </li>
      ))}
    </ul>
  );
}
