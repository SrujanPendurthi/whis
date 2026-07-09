import { useWhis } from "../../../shared/state/WhisContext";

/** Workspace-scoped slice of the app store: ingested sources + ingest actions. */
export function useWorkspace() {
  const { sources, ingest, refreshSources } = useWhis();
  return { sources, ingest, refreshSources };
}
