import type { Card } from "../../../shared/types";

interface CardEditorProps {
  card: Card;
  index: number;
  onChange: (card: Card) => void;
  onRemove: () => void;
}

/** Editable front/back/tags for one staged flashcard. */
export function CardEditor({ card, index, onChange, onRemove }: CardEditorProps) {
  return (
    <div className="space-y-2 rounded-lg border border-neutral-800 bg-neutral-900 p-3">
      <div className="flex items-center justify-between">
        <span className="text-[11px] uppercase tracking-wide text-neutral-500">
          Card {index + 1}
        </span>
        <button
          onClick={onRemove}
          className="text-xs text-neutral-500 hover:text-red-400"
          aria-label="Remove card"
        >
          Remove
        </button>
      </div>

      <textarea
        value={card.front}
        onChange={(e) => onChange({ ...card, front: e.target.value })}
        rows={2}
        placeholder="Front"
        className="w-full resize-none rounded-md border border-neutral-700 bg-neutral-950 px-2 py-1 text-sm text-neutral-100 focus:border-blue-500 focus:outline-none"
      />
      <textarea
        value={card.back}
        onChange={(e) => onChange({ ...card, back: e.target.value })}
        rows={2}
        placeholder="Back"
        className="w-full resize-none rounded-md border border-neutral-700 bg-neutral-950 px-2 py-1 text-sm text-neutral-100 focus:border-blue-500 focus:outline-none"
      />
      <input
        value={card.tags.join(", ")}
        onChange={(e) =>
          onChange({
            ...card,
            tags: e.target.value
              .split(",")
              .map((t) => t.trim())
              .filter(Boolean),
          })
        }
        placeholder="tags, comma separated"
        className="w-full rounded-md border border-neutral-700 bg-neutral-950 px-2 py-1 text-xs text-neutral-400 focus:border-blue-500 focus:outline-none"
      />
    </div>
  );
}
