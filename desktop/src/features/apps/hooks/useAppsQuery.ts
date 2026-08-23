import { useQuery } from "@tanstack/react-query";

import { foldAppMetadataHeads } from "@/features/apps/lib/appMetadata";
import type { AppMetadata } from "@/features/apps/types";
import { useRelaySelfQuery } from "@/features/moderation/hooks";
import { relayClient } from "@/shared/api/relayClient";
import { KIND_APP_METADATA } from "@/shared/constants/kinds";
import { useStableMap } from "@/shared/hooks/useStableReference";

const PUBKEY_HEX_RE = /^[0-9a-f]{64}$/i;
const EMPTY_APPS = new Map<string, AppMetadata>();

function isRelayPubkey(value: string | null | undefined): value is string {
  return typeof value === "string" && PUBKEY_HEX_RE.test(value);
}

export function appsQueryKey(relaySelf: string | null | undefined) {
  return ["apps", relaySelf] as const;
}

export function useAppsQuery() {
  const relaySelf = useRelaySelfQuery().data ?? null;
  const query = useQuery({
    queryKey: appsQueryKey(relaySelf),
    enabled: isRelayPubkey(relaySelf),
    queryFn: async () => {
      if (!isRelayPubkey(relaySelf)) {
        return new Map<string, AppMetadata>();
      }
      const events = await relayClient.fetchEvents({
        kinds: [KIND_APP_METADATA],
        authors: [relaySelf],
        limit: 500,
      });
      return foldAppMetadataHeads(events, relaySelf);
    },
  });
  const data = useStableMap(query.data ?? EMPTY_APPS);
  return { ...query, data: data as ReadonlyMap<string, AppMetadata> };
}
