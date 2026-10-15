import { useState } from "react";
import type { ModelProfile } from "../../../shared/types";
import { useWhis } from "../../../shared/state/WhisContext";
import { Button } from "../../../shared/components/Button";
import { cn } from "../../../shared/utils/cn";
import { api } from "../../../shared/tauri";
import { BLANK_PROFILE } from "../presets";
import { ModelEditor } from "./ModelEditor";

interface Editing {
  profile: ModelProfile;
  isNew: boolean;
}

type TestState = { status: "ok" | "fail" | "running"; message?: string };

/** Full CRUD over model profiles: list, add/edit, delete, test, set active. */
export function ModelManager() {
  const { models, activeModelId, saveModel, deleteModel, selectModel } = useWhis();
  const [editing, setEditing] = useState<Editing | null>(null);
  const [tests, setTests] = useState<Record<string, TestState>>({});

  const startNew = (): void =>
    setEditing({ profile: { id: crypto.randomUUID(), ...BLANK_PROFILE }, isNew: true });

  const onSave = async (profile: ModelProfile): Promise<void> => {
    await saveModel(profile);
    // First model added becomes active automatically for a smooth first run.
    if (models.length === 0) await selectModel(profile.id);
    setEditing(null);
  };

  const test = async (id: string): Promise<void> => {
    setTests((t) => ({ ...t, [id]: { status: "running" } }));
    try {
      await api.testModel(id);
      setTests((t) => ({ ...t, [id]: { status: "ok" } }));
    } catch (err) {
      setTests((t) => ({ ...t, [id]: { status: "fail", message: String(err) } }));
    }
  };

  if (editing) {
    return (
      <ModelEditor
        initial={editing.profile}
        isNew={editing.isNew}
        onSave={onSave}
        onCancel={() => setEditing(null)}
      />
    );
  }

  return (
    <div className="space-y-3">
      {models.length === 0 && (
        <p className="text-sm text-neutral-500">
          No models yet. Add a local server or a cloud provider to get started.
        </p>
      )}

      <ul className="space-y-2">
        {models.map((m) => {
          const isActive = m.id === activeModelId;
          const t = tests[m.id];
          return (
            <li
              key={m.id}
              className={cn(
                "rounded-lg border p-3",
                isActive ? "border-blue-500/60 bg-blue-500/5" : "border-neutral-800 bg-neutral-950",
              )}
            >
              <div className="flex items-start justify-between gap-2">
                <button
                  onClick={() => void selectModel(m.id)}
                  className="min-w-0 flex-1 text-left"
                  title="Set as active model"
                >
                  <div className="flex items-center gap-2">
                    <span
                      className={cn(
                        "h-2 w-2 shrink-0 rounded-full",
                        isActive ? "bg-blue-500" : "bg-neutral-700",
                      )}
                    />
                    <span className="truncate text-sm font-medium text-neutral-100">
                      {m.label}
                    </span>
                    {isActive && <span className="text-[10px] text-blue-400">active</span>}
                  </div>
                  <div className="mt-0.5 truncate pl-4 font-mono text-[11px] text-neutral-500">
                    {m.model} · {m.baseUrl}
                    {m.apiKey.trim() !== "" && " · 🔑"}
                  </div>
                </button>

                <div className="flex shrink-0 gap-1">
                  <button
                    onClick={() => void test(m.id)}
                    className="rounded-md border border-neutral-700 px-2 py-1 text-xs text-neutral-300 hover:bg-neutral-800"
                  >
                    {t?.status === "running" ? "…" : "Test"}
                  </button>
                  <button
                    onClick={() => setEditing({ profile: m, isNew: false })}
                    className="rounded-md border border-neutral-700 px-2 py-1 text-xs text-neutral-300 hover:bg-neutral-800"
                  >
                    Edit
                  </button>
                  <button
                    onClick={() => void deleteModel(m.id)}
                    className="rounded-md border border-neutral-700 px-2 py-1 text-xs text-neutral-400 hover:border-red-500/50 hover:text-red-400"
                  >
                    Delete
                  </button>
                </div>
              </div>

              {t && t.status !== "running" && (
                <p
                  className={cn(
                    "mt-2 pl-4 text-[11px]",
                    t.status === "ok" ? "text-emerald-400" : "text-red-400",
                  )}
                >
                  {t.status === "ok" ? "✓ Reachable" : `✕ ${t.message}`}
                </p>
              )}
            </li>
          );
        })}
      </ul>

      <div className="flex justify-end">
        <Button onClick={startNew}>+ Add model</Button>
      </div>
    </div>
  );
}
