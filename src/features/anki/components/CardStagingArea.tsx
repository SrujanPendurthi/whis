import { useState } from "react";
import { CardEditor } from "./CardEditor";
import { useWhis } from "../../../shared/state/WhisContext";
import { cn } from "../../../shared/utils/cn";

/** Generate → review/edit → sync flashcards to Anki. */
export function CardStagingArea() {
  const { stagedCards, generateCards, updateCard, removeCard, syncCards, ankiOnline, refreshAnki } =
    useWhis();
  const [topic, setTopic] = useState("");
  const [deck, setDeck] = useState("whis");
  const [generating, setGenerating] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [status, setStatus] = useState<string | null>(null);

  const generate = async (): Promise<void> => {
    if (!topic.trim() || generating) return;
    setGenerating(true);
    setStatus(null);
    try {
      await generateCards(topic);
    } catch (err) {
      setStatus(`Generation failed: ${err}`);
    } finally {
      setGenerating(false);
    }
  };

  const sync = async (): Promise<void> => {
    if (stagedCards.length === 0 || syncing) return;
    setSyncing(true);
    setStatus(null);
    try {
      const results = await syncCards(deck);
      const added = results.filter((r) => r !== null).length;
      setStatus(`Synced ${added}/${results.length} card(s) to "${deck}".`);
    } catch (err) {
      setStatus(`Sync failed: ${err}`);
    } finally {
      setSyncing(false);
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="shrink-0 space-y-2 border-b border-neutral-800 p-3">
        <div className="flex items-center justify-between">
          <span className="text-[11px] uppercase tracking-wide text-neutral-500">Anki</span>
          <button
            onClick={() => void refreshAnki()}
            className="flex items-center gap-1.5 text-[11px] text-neutral-500 hover:text-neutral-300"
            title="Click to re-check AnkiConnect"
          >
            <span
              className={cn("h-2 w-2 rounded-full", ankiOnline ? "bg-emerald-500" : "bg-neutral-600")}
            />
            {ankiOnline ? "connected" : "offline"}
          </button>
        </div>

        <div className="flex gap-1">
          <input
            value={topic}
            onChange={(e) => setTopic(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void generate();
            }}
            placeholder="Topic to make cards from…"
            className="min-w-0 flex-1 rounded-md border border-neutral-700 bg-neutral-900 px-2 py-1 text-xs text-neutral-100 placeholder:text-neutral-600 focus:border-blue-500 focus:outline-none"
          />
          <button
            onClick={() => void generate()}
            disabled={generating || !topic.trim()}
            className="rounded-md bg-neutral-800 px-2 py-1 text-xs text-neutral-200 hover:bg-neutral-700 disabled:opacity-40"
          >
            {generating ? "…" : "Generate"}
          </button>
        </div>
      </div>

      <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {stagedCards.length === 0 ? (
          <p className="text-xs text-neutral-600">
            No staged cards. Generate from a topic above, review them here, then sync to Anki.
          </p>
        ) : (
          stagedCards.map((card, i) => (
            <CardEditor
              key={i}
              index={i}
              card={card}
              onChange={(c) => updateCard(i, c)}
              onRemove={() => removeCard(i)}
            />
          ))
        )}
      </div>

      <div className="shrink-0 space-y-2 border-t border-neutral-800 p-3">
        {status && <p className="text-[11px] text-neutral-400">{status}</p>}
        <div className="flex gap-1">
          <input
            value={deck}
            onChange={(e) => setDeck(e.target.value)}
            placeholder="Deck name"
            className="min-w-0 flex-1 rounded-md border border-neutral-700 bg-neutral-900 px-2 py-1 text-xs text-neutral-100 focus:border-blue-500 focus:outline-none"
          />
          <button
            onClick={() => void sync()}
            disabled={syncing || stagedCards.length === 0 || !deck.trim()}
            className="rounded-md bg-blue-600 px-3 py-1 text-xs font-medium text-white hover:bg-blue-500 disabled:bg-neutral-700 disabled:text-neutral-400"
          >
            {syncing ? "Syncing…" : `Sync ${stagedCards.length || ""}`}
          </button>
        </div>
      </div>
    </div>
  );
}
