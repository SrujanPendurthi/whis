import { Sidebar } from "./features/workspace/components/Sidebar";
import { ChatThread } from "./features/chat/components/ChatThread";
import { CommandBar } from "./features/chat/components/CommandBar";
import { PlanViewer } from "./features/study-planner/components/PlanViewer";
import { ResearchTerminal } from "./features/deep-research/components/ResearchTerminal";
import { CardStagingArea } from "./features/anki/components/CardStagingArea";
import { useWhis } from "./shared/state/WhisContext";
import { cn } from "./shared/utils/cn";
import type { RightTab } from "./shared/types";

const TABS: Array<{ id: RightTab; label: string }> = [
  { id: "plan", label: "Plan" },
  { id: "research", label: "Research" },
  { id: "anki", label: "Anki" },
];

/** Right panel: tab bar + the active auxiliary view. */
function RightPanel() {
  const { rightTab, setRightTab } = useWhis();
  return (
    <section className="flex h-full min-h-0 flex-col border-l border-neutral-800 bg-neutral-950">
      <div className="flex shrink-0 border-b border-neutral-800">
        {TABS.map((tab) => (
          <button
            key={tab.id}
            onClick={() => setRightTab(tab.id)}
            className={cn(
              "flex-1 px-3 py-2 text-xs font-medium transition-colors",
              rightTab === tab.id
                ? "border-b-2 border-blue-500 text-neutral-100"
                : "text-neutral-500 hover:text-neutral-300",
            )}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-hidden">
        {rightTab === "plan" && <PlanViewer />}
        {rightTab === "research" && <ResearchTerminal />}
        {rightTab === "anki" && <CardStagingArea />}
      </div>
    </section>
  );
}

/**
 * App shell — three regions per the PRD: workspace sidebar · chat · tabbed
 * auxiliary panel.
 */
function App() {
  return (
    <div className="grid h-full grid-cols-[260px_minmax(0,1fr)_380px] bg-neutral-950 text-neutral-100">
      <Sidebar />
      <main className="flex min-h-0 flex-col">
        <ChatThread />
        <CommandBar />
      </main>
      <RightPanel />
    </div>
  );
}

export default App;
