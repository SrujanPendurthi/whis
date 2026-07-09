/**
 * TypeScript mirror of the Rust IPC payload types in
 * `src-tauri/src/shared/models.rs`. Keep the two in sync — these are the only
 * shapes allowed to cross the Tauri boundary (no `any`).
 */

export type Role = "system" | "user" | "assistant";

export interface ChatMessage {
  role: Role;
  content: string;
}

export interface ChunkMetadata {
  sourceFile: string;
  timestamp: number;
  chunkId: string;
}

/** A retrieved chunk (metadata is flattened onto the object in Rust). */
export interface RetrievedChunk extends ChunkMetadata {
  text: string;
  score: number;
}

/** A distinct ingested source and its chunk count (workspace sidebar). */
export interface SourceInfo {
  sourceFile: string;
  chunkCount: number;
}

export interface Card {
  front: string;
  back: string;
  tags: string[];
}

/** Discriminated union mirroring Rust's `StreamEvent` (serde tag = "type"). */
export type StreamEvent =
  | { type: "token"; text: string }
  | { type: "log"; line: string }
  | { type: "done" }
  | { type: "error"; message: string };

// --- Frontend-only view types ----------------------------------------------

/** The command modes recognized by the router (mirrors Rust `Command`). */
export type Command = "chat" | "plan" | "deep-research" | "socratic";

/** A rendered turn in the chat thread. */
export interface ChatTurn {
  id: string;
  role: "user" | "assistant";
  text: string;
  /** True while tokens are still streaming into this turn. */
  streaming: boolean;
}

/** Which right-panel tab is active. */
export type RightTab = "plan" | "research" | "anki";
