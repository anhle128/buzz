import { resolveAppActor } from "@/features/apps/lib/appActor";
import type { AppMetadata, ResolvedMessageActor } from "@/features/apps/types";
import type { SearchHit } from "@/shared/api/types";

export function resolveSearchHitActor(input: {
  hit: SearchHit;
  apps: ReadonlyMap<string, AppMetadata>;
  relaySelfPubkey?: string | null;
}): ResolvedMessageActor {
  const { hit, apps, relaySelfPubkey } = input;
  return resolveAppActor({
    event: {
      id: hit.eventId,
      pubkey: hit.pubkey,
      created_at: hit.createdAt,
      kind: hit.kind,
      tags: hit.tags,
      content: hit.content,
      sig: "",
    },
    apps,
    relaySelfPubkey,
    mode: "search",
  });
}
