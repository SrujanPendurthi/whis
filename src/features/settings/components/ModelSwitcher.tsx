import { useState } from "react";
import { useWhis } from "../../../shared/state/WhisContext";
import { Modal } from "../../../shared/components/Modal";
import { cn } from "../../../shared/utils/cn";
import { ModelManager } from "./ModelManager";

/**
 * Compact active-model dropdown for the chat header: quick-switch between
 * configured profiles, or open the manager to add/edit them.
 */
export function ModelSwitcher() {
  const { models, activeModelId, selectModel } = useWhis();
  const [open, setOpen] = useState(false);
  const [manage, setManage] = useState(false);

  const active = models.find((m) => m.id === activeModelId) ?? models[0];

  return (
    <>
      <div className="relative">
        <button
          onClick={() => setOpen((o) => !o)}
          className="flex items-center gap-1.5 rounded-md border border-neutral-700 bg-neutral-900 px-2 py-1 text-xs text-neutral-200 hover:bg-neutral-800"
          title="Switch inference model"
        >
          <span className="h-2 w-2 rounded-full bg-blue-500" />
          <span className="max-w-[160px] truncate">{active ? active.label : "No model"}</span>
          <span className="text-neutral-500">▾</span>
        </button>

        {open && (
          <>
            <div className="fixed inset-0 z-10" onClick={() => setOpen(false)} />
            <ul className="absolute right-0 top-full z-20 mt-1 w-64 overflow-hidden rounded-lg border border-neutral-700 bg-neutral-900 shadow-xl">
              {models.length === 0 && (
                <li className="px-3 py-2 text-xs text-neutral-500">No models configured</li>
              )}
              {models.map((m) => (
                <li key={m.id}>
                  <button
                    onClick={() => {
                      void selectModel(m.id);
                      setOpen(false);
                    }}
                    className={cn(
                      "flex w-full items-center gap-2 px-3 py-2 text-left text-sm hover:bg-neutral-800",
                      m.id === active?.id && "bg-neutral-800/60",
                    )}
                  >
                    <span
                      className={cn(
                        "h-2 w-2 shrink-0 rounded-full",
                        m.id === active?.id ? "bg-blue-500" : "bg-neutral-700",
                      )}
                    />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-neutral-100">{m.label}</span>
                      <span className="block truncate font-mono text-[10px] text-neutral-500">
                        {m.model}
                      </span>
                    </span>
                  </button>
                </li>
              ))}
              <li className="border-t border-neutral-800">
                <button
                  onClick={() => {
                    setManage(true);
                    setOpen(false);
                  }}
                  className="w-full px-3 py-2 text-left text-xs text-blue-400 hover:bg-neutral-800"
                >
                  Manage models…
                </button>
              </li>
            </ul>
          </>
        )}
      </div>

      <Modal open={manage} title="Models" onClose={() => setManage(false)}>
        <ModelManager />
      </Modal>
    </>
  );
}
