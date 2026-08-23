import { verifyEvent } from "nostr-tools/pure";

import { parseCanonicalAppId } from "@/features/apps/lib/appMetadata";
import type {
  AppActorMode,
  AppMetadata,
  ResolvedMessageActor,
} from "@/features/apps/types";
import { KIND_STREAM_MESSAGE } from "@/shared/constants/kinds";
import { resolveEventAuthorPubkey } from "@/shared/lib/authors";
import { normalizePubkey } from "@/shared/lib/pubkey";

const PUBKEY_HEX_RE = /^[0-9a-f]{64}$/i;

type ActorEvent = {
  id: string;
  pubkey: string;
  created_at: number;
  kind: number;
  tags: string[][];
  content: string;
  sig: string;
};

function normalizeRelayPubkey(pubkey: string | null | undefined) {
  if (!pubkey) {
    return null;
  }
  const normalized = normalizePubkey(pubkey);
  return PUBKEY_HEX_RE.test(normalized) ? normalized : null;
}

function uniqueAppId(tags: string[][]): string | null {
  let found: string | undefined;
  for (const tag of tags) {
    if (tag[0] !== "buzz:app") {
      continue;
    }
    if (tag.length !== 2) {
      return null;
    }
    if (found !== undefined) {
      return null;
    }
    found = tag[1];
  }
  return parseCanonicalAppId(found);
}

function hasValidSignature(event: ActorEvent) {
  try {
    return verifyEvent(event);
  } catch {
    return false;
  }
}

function fallbackUserActor(
  event: ActorEvent,
  relaySelfPubkey: string | null | undefined,
  attemptedApp: boolean,
): ResolvedMessageActor {
  if (attemptedApp) {
    return { type: "user", pubkey: normalizePubkey(event.pubkey) };
  }
  return {
    type: "user",
    pubkey: resolveEventAuthorPubkey({
      event,
      preferActorTag: true,
      relaySelfPubkey,
      requireChannelTagForPTags: true,
    }),
  };
}

export function resolveAppActor(input: {
  event: ActorEvent;
  apps: ReadonlyMap<string, AppMetadata>;
  relaySelfPubkey?: string | null;
  mode?: AppActorMode;
}): ResolvedMessageActor {
  const { event, apps, relaySelfPubkey, mode = "live" } = input;
  const attemptedApp = event.tags.some((tag) => tag[0] === "buzz:app");
  const requireSignature = mode === "live";
  const relayPubkey = normalizeRelayPubkey(relaySelfPubkey);
  const appId = uniqueAppId(event.tags);
  const metadata = appId ? apps.get(appId) : undefined;

  if (
    event.kind === KIND_STREAM_MESSAGE &&
    relayPubkey &&
    normalizePubkey(event.pubkey) === relayPubkey &&
    (!requireSignature || hasValidSignature(event)) &&
    appId &&
    metadata &&
    metadata.appId === appId &&
    normalizeRelayPubkey(metadata.relayPubkey) === relayPubkey
  ) {
    return {
      type: "app",
      appId: metadata.appId,
      name: metadata.name,
      ...(metadata.picture ? { picture: metadata.picture } : {}),
      signerPubkey: relayPubkey,
    };
  }

  return fallbackUserActor(event, relaySelfPubkey, attemptedApp);
}
