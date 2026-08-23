import { getTextPayload } from "@/shared/api/relayClientShared";

export type ParsedRelayFrame =
  | { type: "auth"; challenge: string }
  | { type: "event"; subId: string; event: unknown }
  | { type: "ok"; eventId: string; success: boolean; message: string }
  | { type: "eose"; subId: string }
  | { type: "closed"; subId: string; message: string }
  | { type: "notice"; notice: string };

function parseFrameArray(data: unknown): ParsedRelayFrame | null {
  if (!Array.isArray(data) || data.length === 0) {
    return null;
  }
  const [type, ...rest] = data;
  if (type === "AUTH" && typeof rest[0] === "string") {
    return { type: "auth", challenge: rest[0] };
  }
  if (type === "EVENT" && typeof rest[0] === "string" && rest[1]) {
    return { type: "event", subId: rest[0], event: rest[1] };
  }
  if (
    type === "OK" &&
    typeof rest[0] === "string" &&
    typeof rest[1] === "boolean"
  ) {
    return {
      type: "ok",
      eventId: rest[0],
      success: rest[1],
      message: typeof rest[2] === "string" ? rest[2] : "",
    };
  }
  if (type === "EOSE" && typeof rest[0] === "string") {
    return { type: "eose", subId: rest[0] };
  }
  if (type === "CLOSED" && typeof rest[0] === "string") {
    return {
      type: "closed",
      subId: rest[0],
      message: typeof rest[1] === "string" ? rest[1] : "",
    };
  }
  if (type === "NOTICE" && typeof rest[0] === "string") {
    return { type: "notice", notice: rest[0] };
  }
  return null;
}

export function parseRelayInboundFrame(
  message: unknown,
): ParsedRelayFrame | null {
  const text = getTextPayload(message);
  if (text !== null) {
    try {
      return parseFrameArray(JSON.parse(text));
    } catch {
      return null;
    }
  }
  return parseFrameArray(message);
}
