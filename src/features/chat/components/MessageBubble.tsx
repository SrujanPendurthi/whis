import { Markdown } from "../../../shared/components/Markdown";
import { cn } from "../../../shared/utils/cn";
import type { ChatTurn } from "../../../shared/types";

/** One chat turn. User turns are plain text; assistant turns render markdown. */
export function MessageBubble({ turn }: { turn: ChatTurn }) {
  const isUser = turn.role === "user";
  return (
    <div className={cn("flex", isUser ? "justify-end" : "justify-start")}>
      <div
        className={cn(
          "max-w-[85%] rounded-2xl px-4 py-2.5 text-sm",
          isUser
            ? "bg-blue-600 text-white"
            : "bg-neutral-800 text-neutral-100",
        )}
      >
        {isUser ? (
          <span className="whitespace-pre-wrap">{turn.text}</span>
        ) : (
          <>
            {turn.text ? <Markdown>{turn.text}</Markdown> : null}
            {turn.streaming && (
              <span className="ml-0.5 inline-block h-4 w-1.5 animate-pulse bg-neutral-400 align-middle" />
            )}
          </>
        )}
      </div>
    </div>
  );
}
