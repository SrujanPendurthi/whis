import { Markdown } from "../../../shared/components/Markdown";
import { useWhis } from "../../../shared/state/WhisContext";

/** Renders the latest `/plan` markdown output. */
export function PlanViewer() {
  const { planText } = useWhis();

  if (!planText) {
    return (
      <p className="p-4 text-xs text-neutral-600">
        Run <code className="text-blue-400">/plan &lt;topic&gt;</code> to generate a study syllabus
        from your workspace. It will appear here.
      </p>
    );
  }

  return (
    <div className="overflow-y-auto p-4 text-neutral-200">
      <Markdown>{planText}</Markdown>
    </div>
  );
}
