import { resolveAppActor } from "@/features/apps/lib/appActor";
import type { AppMetadata, ResolvedMessageActor } from "@/features/apps/types";
import type { FeedItem } from "@/shared/api/types";

export function resolveFeedActor(input: {
  item: FeedItem;
  apps: ReadonlyMap<string, AppMetadata>;
  relaySelfPubkey?: string | null;
}): ResolvedMessageActor {
  const { item, apps, relaySelfPubkey } = input;
  return resolveAppActor({
    event: {
      id: item.id,
      pubkey: item.pubkey,
      created_at: item.createdAt,
      kind: item.kind,
      tags: item.tags,
      content: item.content,
      sig: "",
    },
    apps,
    relaySelfPubkey,
    mode: "feed",
  });
}
