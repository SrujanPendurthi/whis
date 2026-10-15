import { useState } from "react";
import type { ModelProfile } from "../../../shared/types";
import { Button } from "../../../shared/components/Button";
import { PRESETS } from "../presets";

interface ModelEditorProps {
  initial: ModelProfile;
  isNew: boolean;
  onSave: (profile: ModelProfile) => Promise<void>;
  onCancel: () => void;
}

const inputClass =
  "w-full rounded-md border border-neutral-700 bg-neutral-950 px-2 py-1.5 text-sm text-neutral-100 placeholder:text-neutral-600 focus:border-blue-500 focus:outline-none";

const labelClass = "mb-1 block text-[11px] uppercase tracking-wide text-neutral-500";

/** Create/edit form for a single model profile. */
export function ModelEditor({ initial, isNew, onSave, onCancel }: ModelEditorProps) {
  const [draft, setDraft] = useState<ModelProfile>(initial);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);

  const set = <K extends keyof ModelProfile>(key: K, value: ModelProfile[K]): void =>
    setDraft((d) => ({ ...d, [key]: value }));

  const applyPreset = (name: string): void => {
    const preset = PRESETS.find((p) => p.name === name);
    if (preset) setDraft((d) => ({ ...d, ...preset.profile })); // keep id + any typed key
  };

  const valid =
    draft.label.trim() !== "" && draft.baseUrl.trim() !== "" && draft.model.trim() !== "";

  const save = async (): Promise<void> => {
    if (!valid || saving) return;
    setSaving(true);
    setError(null);
    try {
      await onSave(draft);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-3">
      {isNew && (
        <div>
          <label className={labelClass}>Start from a preset</label>
          <select
            defaultValue=""
            onChange={(e) => applyPreset(e.target.value)}
            className={inputClass}
          >
            <option value="" disabled>
              Choose a provider…
            </option>
            {PRESETS.map((p) => (
              <option key={p.name} value={p.name}>
                {p.name}
              </option>
            ))}
          </select>
        </div>
      )}

      <div>
        <label className={labelClass}>Display name</label>
        <input
          value={draft.label}
          onChange={(e) => set("label", e.target.value)}
          placeholder="e.g. GPT-4o"
          className={inputClass}
        />
      </div>

      <div>
        <label className={labelClass}>Base URL (OpenAI-compatible, incl. /v1)</label>
        <input
          value={draft.baseUrl}
          onChange={(e) => set("baseUrl", e.target.value)}
          placeholder="https://api.openai.com/v1"
          className={inputClass}
        />
      </div>

      <div>
        <label className={labelClass}>Model name</label>
        <input
          value={draft.model}
          onChange={(e) => set("model", e.target.value)}
          placeholder="gpt-4o"
          className={inputClass}
        />
      </div>

      <div>
        <label className={labelClass}>API key (leave blank for local)</label>
        <div className="flex gap-1">
          <input
            type={showKey ? "text" : "password"}
            value={draft.apiKey}
            onChange={(e) => set("apiKey", e.target.value)}
            placeholder="sk-…"
            autoComplete="off"
            className={inputClass}
          />
          <button
            type="button"
            onClick={() => setShowKey((s) => !s)}
            className="shrink-0 rounded-md border border-neutral-700 px-2 text-xs text-neutral-400 hover:text-neutral-200"
          >
            {showKey ? "Hide" : "Show"}
          </button>
        </div>
      </div>

      <div>
        <label className={labelClass}>Temperature</label>
        <input
          type="number"
          min={0}
          max={2}
          step={0.1}
          value={draft.temperature}
          onChange={(e) => set("temperature", Number(e.target.value))}
          className={inputClass}
        />
      </div>

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="flex justify-end gap-2 pt-1">
        <Button variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button onClick={() => void save()} disabled={!valid || saving}>
          {saving ? "Saving…" : "Save"}
        </Button>
      </div>
    </div>
  );
}
