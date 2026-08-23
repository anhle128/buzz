import { getEventHash, verifyEvent } from "nostr-tools/pure";

import type { AppMetadata } from "@/features/apps/types";
import type { RelayEvent } from "@/shared/api/types";
import { KIND_APP_METADATA } from "@/shared/constants/kinds";
import { normalizePubkey } from "@/shared/lib/pubkey";

const PUBKEY_HEX_RE = /^[0-9a-f]{64}$/i;

type MetadataEvent = Pick<
  RelayEvent,
  "id" | "pubkey" | "created_at" | "kind" | "tags" | "content" | "sig"
>;

function normalizeRelayPubkey(pubkey: string | null | undefined) {
  if (!pubkey) {
    return null;
  }
  const normalized = normalizePubkey(pubkey);
  return PUBKEY_HEX_RE.test(normalized) ? normalized : null;
}

export function parseCanonicalAppId(value: string | undefined): string | null {
  if (value?.length !== 36) {
    return null;
  }
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (index === 8 || index === 13 || index === 18 || index === 23) {
      if (code !== 45) {
        return null;
      }
      continue;
    }
    const isDigit = code >= 48 && code <= 57;
    const isLowerHex = code >= 97 && code <= 102;
    if (!isDigit && !isLowerHex) {
      return null;
    }
  }
  return value;
}

function uniqueTag(
  tags: string[][],
  name: string,
): { ok: true; value: string | undefined } | { ok: false } {
  let found: string | undefined;
  for (const tag of tags) {
    if (tag[0] !== name) {
      continue;
    }
    if (tag.length !== 2) {
      return { ok: false };
    }
    if (found !== undefined) {
      return { ok: false };
    }
    found = tag[1];
  }
  return { ok: true, value: found };
}

function hasValidEventIdAndSignature(event: MetadataEvent) {
  try {
    if (getEventHash(event) !== event.id) {
      return false;
    }
    return verifyEvent(event);
  } catch {
    return false;
  }
}

export function parseAppMetadata(
  event: MetadataEvent,
  relaySelfPubkey: string | null | undefined,
): AppMetadata | null {
  const relayPubkey = normalizeRelayPubkey(relaySelfPubkey);
  if (!relayPubkey || event.kind !== KIND_APP_METADATA) {
    return null;
  }
  if (normalizePubkey(event.pubkey) !== relayPubkey) {
    return null;
  }
  if (!hasValidEventIdAndSignature(event)) {
    return null;
  }

  const dTag = uniqueTag(event.tags, "d");
  const nameTag = uniqueTag(event.tags, "name");
  const statusTag = uniqueTag(event.tags, "status");
  const pictureTag = uniqueTag(event.tags, "picture");
  if (!dTag.ok || !nameTag.ok || !statusTag.ok || !pictureTag.ok) {
    return null;
  }

  const appId = parseCanonicalAppId(dTag.value);
  const name = nameTag.value;
  const status = statusTag.value;
  if (!appId || !name || (status !== "active" && status !== "disabled")) {
    return null;
  }

  const metadata: AppMetadata = {
    appId,
    name,
    status,
    eventId: event.id,
    relayPubkey,
    updatedAt: event.created_at,
  };
  if (event.content) {
    metadata.description = event.content;
  }
  if (pictureTag.value) {
    metadata.picture = pictureTag.value;
  }
  return metadata;
}

function isNewerHead(candidate: MetadataEvent, current: MetadataEvent) {
  if (candidate.created_at !== current.created_at) {
    return candidate.created_at > current.created_at;
  }
  return candidate.id < current.id;
}

export function foldAppMetadataHeads(
  events: readonly MetadataEvent[],
  relaySelfPubkey: string | null | undefined,
): Map<string, AppMetadata> {
  const heads = new Map<
    string,
    { event: MetadataEvent; metadata: AppMetadata }
  >();
  for (const event of events) {
    const metadata = parseAppMetadata(event, relaySelfPubkey);
    if (!metadata) {
      continue;
    }
    const current = heads.get(metadata.appId);
    if (current && !isNewerHead(event, current.event)) {
      continue;
    }
    heads.set(metadata.appId, { event, metadata });
  }

  const folded = new Map<string, AppMetadata>();
  for (const [appId, head] of heads) {
    folded.set(appId, head.metadata);
  }
  return folded;
}
