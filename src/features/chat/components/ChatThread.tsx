import { useEffect, useRef } from "react";
import { MessageBubble } from "./MessageBubble";
import { useChatStream } from "../hooks/useChatStream";

const HINTS: Array<[string, string]> = [
  ["ask anything", "grounded answers from your documents"],
  ["/plan", "generate a structured study syllabus"],
  ["/deep-research", "run an iterative investigation"],
  ["/socratic", "be challenged with a probing question"],
];

/** Scrollable conversation thread with an empty-state cheat sheet. */
export function ChatThread() {
  const { messages } = useChatStream();
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    endRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  if (messages.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-4 p-8 text-center">
        <div className="text-2xl font-semibold text-neutral-300">whis</div>
        <p className="max-w-sm text-sm text-neutral-500">
          Your private, local study companion. Add documents on the left, then:
        </p>
        <ul className="space-y-1 text-sm">
          {HINTS.map(([cmd, desc]) => (
            <li key={cmd} className="text-neutral-400">
              <code className="rounded bg-neutral-800 px-1.5 py-0.5 text-blue-400">{cmd}</code>
              <span className="ml-2 text-neutral-600">{desc}</span>
            </li>
          ))}
        </ul>
      </div>
    );
  }

  return (
    <div className="flex-1 space-y-4 overflow-y-auto px-6 py-6">
      {messages.map((turn) => (
        <MessageBubble key={turn.id} turn={turn} />
      ))}
      <div ref={endRef} />
    </div>
  );
}
