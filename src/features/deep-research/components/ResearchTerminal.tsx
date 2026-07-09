import { useEffect, useRef } from "react";
import { Markdown } from "../../../shared/components/Markdown";
import { useWhis } from "../../../shared/state/WhisContext";

/** Live research logs (terminal) above the synthesized markdown report. */
export function ResearchTerminal() {
  const { research } = useWhis();
  const logsEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    logsEndRef.current?.scrollIntoView();
  }, [research.logs]);

  const empty = research.logs.length === 0 && !research.report;
  if (empty) {
    return (
      <p className="p-4 text-xs text-neutral-600">
        Run <code className="text-blue-400">/deep-research &lt;topic&gt;</code> to launch an
        investigation. Progress logs and the final report will stream here.
      </p>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {research.logs.length > 0 && (
        <div className="max-h-48 shrink-0 overflow-y-auto border-b border-neutral-800 bg-black/40 p-3 font-mono text-[11px] leading-relaxed text-emerald-400">
          {research.logs.map((line, i) => (
            <div key={i} className="whitespace-pre-wrap">
              <span className="text-neutral-600">›</span> {line}
            </div>
          ))}
          <div ref={logsEndRef} />
        </div>
      )}
      {research.report && (
        <div className="min-h-0 flex-1 overflow-y-auto p-4 text-neutral-200">
          <Markdown>{research.report}</Markdown>
        </div>
      )}
    </div>
  );
}
