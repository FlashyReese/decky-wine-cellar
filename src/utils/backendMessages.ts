import { MessageEnvelope, MessageType } from "../types";
import { error } from "./logger";

const validMessageTypes = new Set<string>(Object.values(MessageType));

export function parseBackendMessage(
  data: unknown,
): MessageEnvelope | undefined {
  if (typeof data !== "string") {
    error("Received non-string backend message");
    return undefined;
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(data);
  } catch (err) {
    error("Failed to parse backend message:", err);
    return undefined;
  }

  if (
    parsed == null ||
    typeof parsed !== "object" ||
    !("type" in parsed) ||
    typeof parsed.type !== "string" ||
    !validMessageTypes.has(parsed.type)
  ) {
    error("Received invalid backend message shape:", parsed);
    return undefined;
  }

  return parsed as MessageEnvelope;
}
