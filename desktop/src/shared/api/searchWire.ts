import type { SearchHit } from "@/shared/api/searchTypes";

export type RawSearchHit = {
  event_id: string;
  content: string;
  kind: number;
  pubkey: string;
  channel_id: string | null;
  channel_name: string | null;
  created_at: number;
  score: number;
  tags?: string[][];
};

export type RawSearchResponse = {
  hits: RawSearchHit[];
  found: number;
};

export function fromRawSearchHit(hit: RawSearchHit): SearchHit {
  return {
    eventId: hit.event_id,
    content: hit.content,
    kind: hit.kind,
    pubkey: hit.pubkey,
    channelId: hit.channel_id,
    channelName: hit.channel_name,
    createdAt: hit.created_at,
    score: hit.score,
    tags: hit.tags ?? [],
  };
}
