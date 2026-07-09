import { useWhis } from "../../../shared/state/WhisContext";

/** Chat-scoped slice of the app store: the thread, busy flag, and submit action. */
export function useChatStream() {
  const { messages, busy, submit } = useWhis();
  return { messages, busy, submit };
}
