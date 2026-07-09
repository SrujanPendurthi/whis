/**
 * Typed wrappers around the Tauri IPC surface. Every backend command is exposed
 * here with concrete types so no `any` leaks across the boundary.
 */
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Card, SourceInfo, StreamEvent } from "./types";

export interface StreamHandlers {
  onToken?: (text: string) => void;
  onLog?: (line: string) => void;
  onError?: (message: string) => void;
}

/**
 * Run a router command, dispatching streamed events to the handlers. Resolves
 * when the stream terminates (on `done` OR `error`); errors are surfaced via
 * `onError` rather than a rejection, so callers have a single completion path.
 */
export async function streamCommand(
  input: string,
  handlers: StreamHandlers,
): Promise<void> {
  const channel = `whis-${crypto.randomUUID()}`;

  return new Promise<void>((resolve) => {
    let unlisten: (() => void) | null = null;
    let settled = false;
    const finish = (fn?: () => void): void => {
      if (settled) return;
      settled = true;
      unlisten?.();
      fn?.();
      resolve();
    };

    listen<StreamEvent>(channel, (event) => {
      const payload = event.payload;
      switch (payload.type) {
        case "token":
          handlers.onToken?.(payload.text);
          break;
        case "log":
          handlers.onLog?.(payload.line);
          break;
        case "done":
          finish();
          break;
        case "error":
          finish(() => handlers.onError?.(payload.message));
          break;
      }
    })
      .then((fn) => {
        unlisten = fn;
        // The command may reject before emitting any event (e.g. model init
        // failed). Surface that too, exactly once.
        invoke("run_command", { channel, input }).catch((err: unknown) => {
          finish(() => handlers.onError?.(String(err)));
        });
      })
      .catch((err: unknown) => {
        finish(() => handlers.onError?.(String(err)));
      });
  });
}

/** Direct (non-streaming) command wrappers. */
export const api = {
  ingestFile: (path: string): Promise<number> => invoke<number>("ingest_file", { path }),
  ingestWeb: (url: string): Promise<number> => invoke<number>("ingest_web", { url }),
  ingestYoutube: (url: string): Promise<number> => invoke<number>("ingest_youtube", { url }),
  listSources: (): Promise<SourceInfo[]> => invoke<SourceInfo[]>("list_sources"),
  generateCards: (topic: string): Promise<Card[]> => invoke<Card[]>("generate_cards", { topic }),
  ankiStatus: (): Promise<boolean> => invoke<boolean>("anki_status"),
  ankiSync: (deck: string, cards: Card[]): Promise<Array<number | null>> =>
    invoke<Array<number | null>>("anki_sync_cards", { deck, cards }),
  serviceEndpoints: (): Promise<Record<string, string>> =>
    invoke<Record<string, string>>("service_endpoints"),
};
