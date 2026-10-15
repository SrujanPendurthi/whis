/**
 * WhisContext — the single source of truth for UI state and the actions that
 * drive the backend. Routing output by command lives here: `/plan` streams into
 * the Plan tab, `/deep-research` into the Research terminal, and plain chat /
 * `/socratic` into the message thread.
 */
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { api, streamCommand } from "../tauri";
import type { Card, ChatTurn, Command, ModelProfile, RightTab, SourceInfo } from "../types";

interface ResearchState {
  logs: string[];
  report: string;
}

interface WhisState {
  messages: ChatTurn[];
  busy: boolean;
  planText: string;
  research: ResearchState;
  rightTab: RightTab;
  sources: SourceInfo[];
  stagedCards: Card[];
  ankiOnline: boolean;
  models: ModelProfile[];
  activeModelId: string | null;
}

interface WhisActions {
  submit: (input: string) => Promise<void>;
  setRightTab: (tab: RightTab) => void;
  refreshSources: () => Promise<void>;
  ingest: (kind: "file" | "web" | "youtube", value: string) => Promise<void>;
  generateCards: (topic: string) => Promise<void>;
  updateCard: (index: number, card: Card) => void;
  removeCard: (index: number) => void;
  syncCards: (deck: string) => Promise<Array<number | null>>;
  refreshAnki: () => Promise<void>;
  refreshModels: () => Promise<void>;
  saveModel: (profile: ModelProfile) => Promise<void>;
  deleteModel: (id: string) => Promise<void>;
  selectModel: (id: string) => Promise<void>;
}

type WhisContextValue = WhisState & WhisActions;

const WhisContext = createContext<WhisContextValue | null>(null);

/** Mirror of the backend router's command parsing (see `router.rs::parse`). */
function parseCommand(input: string): Command {
  const head = input.trim().split(/\s+/)[0] ?? "";
  if (head === "/plan") return "plan";
  if (head === "/deep-research") return "deep-research";
  if (head === "/socratic") return "socratic";
  return "chat";
}

const uid = (): string => crypto.randomUUID();

export function WhisProvider({ children }: { children: ReactNode }) {
  const [messages, setMessages] = useState<ChatTurn[]>([]);
  const [busy, setBusy] = useState(false);
  const [planText, setPlanText] = useState("");
  const [research, setResearch] = useState<ResearchState>({ logs: [], report: "" });
  const [rightTab, setRightTab] = useState<RightTab>("plan");
  const [sources, setSources] = useState<SourceInfo[]>([]);
  const [stagedCards, setStagedCards] = useState<Card[]>([]);
  const [ankiOnline, setAnkiOnline] = useState(false);
  const [models, setModels] = useState<ModelProfile[]>([]);
  const [activeModelId, setActiveModelId] = useState<string | null>(null);

  const busyRef = useRef(busy);
  busyRef.current = busy;

  const patchTurn = useCallback((id: string, fn: (t: ChatTurn) => ChatTurn) => {
    setMessages((prev) => prev.map((m) => (m.id === id ? fn(m) : m)));
  }, []);

  const refreshSources = useCallback(async () => {
    try {
      setSources(await api.listSources());
    } catch {
      // Store may be unavailable (fresh install); leave sources empty.
    }
  }, []);

  const refreshAnki = useCallback(async () => {
    try {
      setAnkiOnline(await api.ankiStatus());
    } catch {
      setAnkiOnline(false);
    }
  }, []);

  const refreshModels = useCallback(async () => {
    try {
      const [list, active] = await Promise.all([api.listModels(), api.activeModel()]);
      setModels(list);
      setActiveModelId(active);
    } catch {
      // Settings unavailable; leave model list empty.
    }
  }, []);

  const selectModel = useCallback(async (id: string) => {
    await api.setActiveModel(id);
    setActiveModelId(id);
  }, []);

  const saveModel = useCallback(
    async (profile: ModelProfile) => {
      await api.saveModel(profile);
      await refreshModels();
    },
    [refreshModels],
  );

  const deleteModel = useCallback(
    async (id: string) => {
      await api.deleteModel(id);
      await refreshModels();
    },
    [refreshModels],
  );

  useEffect(() => {
    void refreshSources();
    void refreshAnki();
    void refreshModels();
  }, [refreshSources, refreshAnki, refreshModels]);

  const submit = useCallback(
    async (input: string): Promise<void> => {
      const text = input.trim();
      if (!text || busyRef.current) return;
      const command = parseCommand(text);
      setBusy(true);

      try {
        if (command === "chat" || command === "socratic") {
          const assistantId = uid();
          setMessages((prev) => [
            ...prev,
            { id: uid(), role: "user", text, streaming: false },
            { id: assistantId, role: "assistant", text: "", streaming: true },
          ]);
          await streamCommand(text, {
            onToken: (t) => patchTurn(assistantId, (m) => ({ ...m, text: m.text + t })),
            onError: (msg) =>
              patchTurn(assistantId, (m) => ({ ...m, text: `${m.text}\n\n⚠️ ${msg}` })),
          });
          patchTurn(assistantId, (m) => ({ ...m, streaming: false }));
        } else if (command === "plan") {
          setRightTab("plan");
          setPlanText("");
          setMessages((prev) => [
            ...prev,
            { id: uid(), role: "user", text, streaming: false },
            {
              id: uid(),
              role: "assistant",
              text: "📋 Building your study plan — see the **Plan** tab →",
              streaming: false,
            },
          ]);
          await streamCommand(text, {
            onToken: (t) => setPlanText((p) => p + t),
            onError: (msg) => setPlanText((p) => `${p}\n\n⚠️ ${msg}`),
          });
        } else {
          // deep-research
          setRightTab("research");
          setResearch({ logs: [], report: "" });
          setMessages((prev) => [
            ...prev,
            { id: uid(), role: "user", text, streaming: false },
            {
              id: uid(),
              role: "assistant",
              text: "🔎 Researching — watch the **Research** tab →",
              streaming: false,
            },
          ]);
          await streamCommand(text, {
            onLog: (line) => setResearch((r) => ({ ...r, logs: [...r.logs, line] })),
            onToken: (t) => setResearch((r) => ({ ...r, report: r.report + t })),
            onError: (msg) =>
              setResearch((r) => ({ ...r, report: `${r.report}\n\n⚠️ ${msg}` })),
          });
        }
      } finally {
        setBusy(false);
      }
    },
    [patchTurn],
  );

  const ingest = useCallback(
    async (kind: "file" | "web" | "youtube", value: string): Promise<void> => {
      const v = value.trim();
      if (!v) return;
      if (kind === "file") await api.ingestFile(v);
      else if (kind === "web") await api.ingestWeb(v);
      else await api.ingestYoutube(v);
      await refreshSources();
    },
    [refreshSources],
  );

  const generateCards = useCallback(async (topic: string): Promise<void> => {
    const cards = await api.generateCards(topic);
    setStagedCards(cards);
  }, []);

  const updateCard = useCallback((index: number, card: Card) => {
    setStagedCards((prev) => prev.map((c, i) => (i === index ? card : c)));
  }, []);

  const removeCard = useCallback((index: number) => {
    setStagedCards((prev) => prev.filter((_, i) => i !== index));
  }, []);

  const syncCards = useCallback(
    async (deck: string): Promise<Array<number | null>> => {
      const result = await api.ankiSync(deck, stagedCards);
      return result;
    },
    [stagedCards],
  );

  const value = useMemo<WhisContextValue>(
    () => ({
      messages,
      busy,
      planText,
      research,
      rightTab,
      sources,
      stagedCards,
      ankiOnline,
      models,
      activeModelId,
      submit,
      setRightTab,
      refreshSources,
      ingest,
      generateCards,
      updateCard,
      removeCard,
      syncCards,
      refreshAnki,
      refreshModels,
      saveModel,
      deleteModel,
      selectModel,
    }),
    [
      messages,
      busy,
      planText,
      research,
      rightTab,
      sources,
      stagedCards,
      ankiOnline,
      models,
      activeModelId,
      submit,
      refreshSources,
      ingest,
      generateCards,
      updateCard,
      removeCard,
      syncCards,
      refreshAnki,
      refreshModels,
      saveModel,
      deleteModel,
      selectModel,
    ],
  );

  return <WhisContext.Provider value={value}>{children}</WhisContext.Provider>;
}

export function useWhis(): WhisContextValue {
  const ctx = useContext(WhisContext);
  if (!ctx) throw new Error("useWhis must be used within a WhisProvider");
  return ctx;
}
