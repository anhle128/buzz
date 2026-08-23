import type { AppMetadata } from "@/features/apps/types";
import {
  resolveUserLabel,
  type UserProfileLookup,
} from "@/features/profile/lib/identity";
import { resolveSearchHitActor } from "@/features/search/lib/searchHitActor";
import { resultTestId } from "@/features/search/ui/SearchResultItem";
import type { Channel, SearchHit } from "@/shared/api/types";
import { cn } from "@/shared/lib/cn";
import { Badge } from "@/shared/ui/badge";
import {
  MENTION_CHIP_BASE_CLASSES,
  MESSAGE_MARKDOWN_CLASS,
} from "@/shared/ui/mentionChip";
import { UserAvatar } from "@/shared/ui/UserAvatar";

export type SearchHitContextLabel = {
  channelLabel: string | null;
  text: string;
};

export function truncateResultText(content: string, maxLength = 96) {
  const trimmed = content.trim();
  if (trimmed.length === 0) {
    return "No message body.";
  }

  if (trimmed.length <= maxLength) {
    return trimmed;
  }

  return `${trimmed.slice(0, maxLength - 3).trimEnd()}...`;
}

export function formatRelativeTime(unixSeconds: number) {
  const diff = Math.floor(Date.now() / 1_000) - unixSeconds;

  if (diff < 60) {
    return "just now";
  }

  if (diff < 60 * 60) {
    return `${Math.floor(diff / 60)}m ago`;
  }

  if (diff < 60 * 60 * 24) {
    return `${Math.floor(diff / (60 * 60))}h ago`;
  }

  if (diff < 60 * 60 * 24 * 7) {
    return `${Math.floor(diff / (60 * 60 * 24))}d ago`;
  }

  return new Intl.DateTimeFormat("en-US", {
    month: "short",
    day: "numeric",
  }).format(new Date(unixSeconds * 1_000));
}

export function getSearchHitChannelName(
  hit: SearchHit,
  channelLookup: ReadonlyMap<string, Channel>,
  channelLabels?: Record<string, string>,
) {
  const channel = hit.channelId ? channelLookup.get(hit.channelId) : null;
  const channelName =
    (hit.channelId ? channelLabels?.[hit.channelId]?.trim() : null) ||
    hit.channelName?.trim() ||
    channel?.name.trim() ||
    null;

  if (!channelName) {
    return null;
  }

  return channelName;
}

export function getSearchHitContextLabel(
  hit: SearchHit,
  channelLookup: ReadonlyMap<string, Channel>,
  channelLabels?: Record<string, string>,
): SearchHitContextLabel {
  const channel = hit.channelId ? channelLookup.get(hit.channelId) : null;
  const channelName = getSearchHitChannelName(
    hit,
    channelLookup,
    channelLabels,
  );

  if (channel?.channelType === "dm") {
    return {
      channelLabel: null,
      text: "Direct message",
    };
  }

  const isThread = hit.kind === 45003 || Boolean(hit.threadRootId);

  return {
    channelLabel: channelName,
    text: channelName
      ? `${isThread ? "Thread" : "Message"} in`
      : isThread
        ? "Thread"
        : "Message",
  };
}

function SearchHitContextLine({ label }: { label: SearchHitContextLabel }) {
  return (
    <span
      className={cn(
        MESSAGE_MARKDOWN_CLASS,
        "mt-0 flex min-w-0 items-center gap-1.5 text-2xs font-medium leading-3 text-muted-foreground/80",
      )}
    >
      <span className="shrink-0">{label.text}</span>
      {label.channelLabel ? (
        <span
          className={cn(
            MENTION_CHIP_BASE_CLASSES,
            "search-channel-chip min-w-0 max-w-full overflow-hidden",
          )}
          data-channel-link=""
        >
          <span className="truncate">#{label.channelLabel}</span>
        </span>
      ) : null}
    </span>
  );
}

export function MessageSearchResultRow({
  apps,
  channelLabels,
  channelLookup,
  currentPubkey,
  hit,
  menuIndex,
  onClick,
  onMouseEnter,
  relaySelfPubkey,
  resultProfiles,
  selected,
}: {
  apps: ReadonlyMap<string, AppMetadata>;
  channelLabels?: Record<string, string>;
  channelLookup: ReadonlyMap<string, Channel>;
  currentPubkey?: string;
  hit: SearchHit;
  menuIndex: number;
  onClick: () => void;
  onMouseEnter: () => void;
  relaySelfPubkey?: string | null;
  resultProfiles?: UserProfileLookup;
  selected: boolean;
}) {
  const actor = resolveSearchHitActor({
    hit,
    apps,
    relaySelfPubkey,
  });
  const messageAuthorLabel =
    actor.type === "app"
      ? actor.name
      : resolveUserLabel({
          currentPubkey,
          profiles: resultProfiles,
          pubkey: actor.pubkey,
          preferResolvedSelfLabel: true,
        });
  const avatarUrl =
    actor.type === "app"
      ? (actor.picture ?? null)
      : (resultProfiles?.[actor.pubkey.toLowerCase()]?.avatarUrl ?? null);
  const messageContextLabel = getSearchHitContextLabel(
    hit,
    channelLookup,
    channelLabels,
  );
  const preview = truncateResultText(hit.content);
  const trailingLabel = formatRelativeTime(hit.createdAt);

  return (
    <button
      aria-selected={selected}
      className={cn(
        "search-result-row flex w-full gap-3 rounded-lg px-2.5 text-left transition-colors",
        "items-start",
        "py-3.5",
        selected ? "bg-muted/45 text-foreground" : "hover:bg-muted/35",
      )}
      onClick={onClick}
      onMouseEnter={onMouseEnter}
      role="option"
      type="button"
      data-testid={resultTestId({ kind: "message", hit })}
      data-search-result-index={menuIndex}
    >
      <UserAvatar
        avatarUrl={avatarUrl}
        className="h-8 w-8"
        displayName={messageAuthorLabel}
        size="md"
      />
      <span className="min-w-0 flex-1">
        <span className="grid w-full min-w-0 grid-cols-[minmax(0,1fr)_auto] gap-x-3">
          <span className="col-start-1 row-start-1 flex min-w-0 items-center gap-1.5">
            <span className="min-w-0 truncate text-sm font-semibold leading-4 text-foreground">
              {messageAuthorLabel}
            </span>
            {actor.type === "app" ? (
              <Badge data-testid="search-app-badge" variant="secondary">
                App
              </Badge>
            ) : null}
          </span>
          {trailingLabel ? (
            <span className="col-start-2 row-start-1 flex shrink-0 items-center justify-self-end text-xs font-medium leading-4 text-muted-foreground/70">
              {trailingLabel}
            </span>
          ) : null}
          {messageContextLabel ? (
            <span className="col-start-1 min-w-0">
              <SearchHitContextLine label={messageContextLabel} />
            </span>
          ) : null}
          {preview ? (
            <span className="col-start-1 mt-1.5 block min-w-0 truncate text-sm leading-5 text-muted-foreground">
              {preview}
            </span>
          ) : null}
        </span>
      </span>
    </button>
  );
}
