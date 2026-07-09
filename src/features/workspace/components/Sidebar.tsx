import { UploadZone } from "./UploadZone";
import { FileTree } from "./FileTree";

/** Left sidebar: brand, ingest controls, and the document list. */
export function Sidebar() {
  return (
    <aside className="flex h-full min-h-0 flex-col border-r border-neutral-800 bg-neutral-950">
      <div className="border-b border-neutral-800 px-4 py-3">
        <h1 className="text-sm font-semibold tracking-wide text-neutral-100">whis</h1>
        <p className="text-[11px] text-neutral-600">local study AI</p>
      </div>

      <div className="border-b border-neutral-800 p-3">
        <UploadZone />
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <p className="px-1 pb-1 text-[11px] uppercase tracking-wide text-neutral-500">
          Workspace
        </p>
        <FileTree />
      </div>
    </aside>
  );
}
