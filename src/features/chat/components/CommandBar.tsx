import { useRef, useState, type KeyboardEvent } from "react";
import { useChatStream } from "../hooks/useChatStream";
import { cn } from "../../../shared/utils/cn";

const COMMANDS: Array<{ cmd: string; desc: string }> = [
  { cmd: "/plan", desc: "Generate a study syllabus" },
  { cmd: "/deep-research", desc: "Iterative research + report" },
  { cmd: "/socratic", desc: "Be challenged with one question" },
];

/**
 * Terminal-like input. Typing `/` surfaces command autosuggestions; Tab/Enter
 * completes the highlighted one, arrows navigate, and Enter otherwise submits.
 */
export function CommandBar() {
  const { submit, busy } = useChatStream();
  const [value, setValue] = useState("");
  const [active, setActive] = useState(0);
  const [dismissed, setDismissed] = useState(false);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  const isTypingCommand = value.startsWith("/") && !value.includes(" ");
  const suggestions =
    isTypingCommand && !dismissed
      ? COMMANDS.filter((c) => c.cmd.startsWith(value) && c.cmd !== value)
      : [];
  const activeIdx = Math.min(active, Math.max(0, suggestions.length - 1));

  const complete = (cmd: string): void => {
    setValue(cmd + " ");
    setDismissed(true);
    setActive(0);
    inputRef.current?.focus();
  };

  const doSubmit = (): void => {
    if (!value.trim() || busy) return;
    const input = value;
    setValue("");
    setDismissed(false);
    setActive(0);
    void submit(input);
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>): void => {
    if (suggestions.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setActive((i) => (i + 1) % suggestions.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setActive((i) => (i - 1 + suggestions.length) % suggestions.length);
        return;
      }
      if (e.key === "Tab" || (e.key === "Enter" && !e.shiftKey)) {
        e.preventDefault();
        const choice = suggestions[activeIdx];
        if (choice) complete(choice.cmd);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setDismissed(true);
        return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      doSubmit();
    }
  };

  return (
    <div className="relative border-t border-neutral-800 bg-neutral-950 p-3">
      {suggestions.length > 0 && (
        <ul className="absolute bottom-full left-3 mb-2 w-72 overflow-hidden rounded-lg border border-neutral-700 bg-neutral-900 shadow-xl">
          {suggestions.map((s, i) => (
            <li key={s.cmd}>
              <button
                onMouseDown={(e) => {
                  e.preventDefault();
                  complete(s.cmd);
                }}
                className={cn(
                  "flex w-full items-baseline gap-2 px-3 py-2 text-left text-sm",
                  i === activeIdx ? "bg-neutral-800" : "hover:bg-neutral-800/60",
                )}
              >
                <span className="font-mono text-blue-400">{s.cmd}</span>
                <span className="text-xs text-neutral-500">{s.desc}</span>
              </button>
            </li>
          ))}
        </ul>
      )}

      <div className="flex items-end gap-2">
        <textarea
          ref={inputRef}
          rows={1}
          value={value}
          onChange={(e) => {
            setValue(e.target.value);
            setDismissed(false);
            setActive(0);
          }}
          onKeyDown={onKeyDown}
          disabled={busy}
          placeholder={busy ? "Working…" : "Ask, or type / for commands…"}
          className="max-h-40 min-h-[2.5rem] flex-1 resize-none rounded-lg border border-neutral-700 bg-neutral-900 px-3 py-2 text-sm text-neutral-100 placeholder:text-neutral-600 focus:border-blue-500 focus:outline-none disabled:opacity-60"
        />
        <button
          onClick={doSubmit}
          disabled={busy || !value.trim()}
          className="h-[2.5rem] rounded-lg bg-blue-600 px-4 text-sm font-medium text-white hover:bg-blue-500 disabled:bg-neutral-700 disabled:text-neutral-400"
        >
          Send
        </button>
      </div>
    </div>
  );
}
