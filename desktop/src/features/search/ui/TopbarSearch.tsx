import { Search } from "lucide-react";
import * as React from "react";

import { useAppsQuery } from "@/features/apps/hooks/useAppsQuery";
import { useRelaySelfQuery } from "@/features/moderation/hooks";
import { getMinimumSearchQueryLength } from "@/features/search/hooks";
import { useSearchResults } from "@/features/search/useSearchResults";
import {
  resultIcon,
  resultKey,
  resultTestId,
  type SearchResult,
} from "@/features/search/ui/SearchResultItem";
import {
  formatRelativeTime,
  MessageSearchResultRow,
} from "@/features/search/ui/MessageSearchResultRow";
import {
  CurrentChannelSearchAction,
  getChannelScopeLabel,
  SearchDialogInputRow,
} from "@/features/search/ui/SearchScopeControls";
import { useSearchMenuKeyboardNavigation } from "@/features/search/ui/useSearchMenuKeyboardNavigation";
import type { Channel, SearchHit, UserSearchResult } from "@/shared/api/types";
import { cn } from "@/shared/lib/cn";
import { normalizePubkey, truncatePubkey } from "@/shared/lib/pubkey";
import { Dialog, DialogContent, DialogTitle } from "@/shared/ui/dialog";
import { useDeferredModalOpen } from "@/shared/ui/deferredModalOpen";
import { Skeleton } from "@/shared/ui/skeleton";
import { UserAvatar } from "@/shared/ui/UserAvatar";

type TopbarSearchProps = {
  channelLabels?: Record<string, string>;
  channels: Channel[];
  className?: string;
  currentPubkey?: string;
  currentChannelId?: string | null;
  focusRequest?: number;
  onOpenChannel: (channelId: string) => void;
  onOpenResult: (hit: SearchHit) => void;
  onOpenUser?: (user: UserSearchResult) => void | Promise<void>;
  onBrowseChannels?: () => void | Promise<void>;
  onCreateAgent?: () => void | Promise<void>;
  onCreateChannel?: () => void | Promise<void>;
  suggestionChannels?: Channel[];
  scopeFocusRequest?: number;
  variant?: "bar" | "icon";
};

const MAX_SEARCH_SUGGESTIONS = 4;
const SEARCH_RESULT_LIMIT = 40;
const SEARCH_SECTION_TITLE_CLASS =
  "px-2.5 pb-1.5 pt-2 text-xs font-medium text-muted-foreground/70";
const SEARCH_RESULT_SECTION_ORDER = [
  "channels",
  "direct-messages",
  "people",
  "agents",
  "messages",
  "actions",
] as const;

type SearchResultSectionKey = (typeof SEARCH_RESULT_SECTION_ORDER)[number];

type SearchResultSection = {
  key: SearchResultSectionKey;
  results: SearchResult[];
  title: string;
};

function getChannelActivityTime(channel: Channel) {
  if (!channel.lastMessageAt) {
    return 0;
  }

  const timestamp = Date.parse(channel.lastMessageAt);
  return Number.isFinite(timestamp) ? timestamp : 0;
}

function getChannelSuggestionMeta(channel: Channel) {
  const activityTime = getChannelActivityTime(channel);

  if (activityTime > 0) {
    return formatRelativeTime(Math.floor(activityTime / 1_000));
  }

  return null;
}

function getChannelDisplayName(
  channel: Channel,
  channelLabels?: Record<string, string>,
) {
  return channelLabels?.[channel.id]?.trim() || channel.name;
}

function getChannelPreview(channel: Channel) {
  if (channel.channelType === "dm") {
    return "";
  }

  if (channel.description.trim()) {
    return channel.description;
  }

  return "";
}

function getUserDisplayName(user: UserSearchResult) {
  return (
    user.displayName?.trim() ||
    user.nip05Handle?.trim() ||
    truncatePubkey(user.pubkey)
  );
}

function getUserSecondaryLabel(user: UserSearchResult) {
  const displayName = user.displayName?.trim();
  const nip05Handle = user.nip05Handle?.trim();

  if (nip05Handle && nip05Handle !== displayName) {
    return nip05Handle;
  }

  return null;
}

function getResultSectionKey(result: SearchResult): SearchResultSectionKey {
  if (result.kind === "channel") {
    return result.channel.channelType === "dm" ? "direct-messages" : "channels";
  }

  if (result.kind === "user") {
    return result.user.isAgent ? "agents" : "people";
  }

  if (result.kind === "action") {
    return "actions";
  }

  return "messages";
}

function getSectionTitle(sectionKey: SearchResultSectionKey) {
  switch (sectionKey) {
    case "channels":
      return "Channels";
    case "direct-messages":
      return "Direct messages";
    case "people":
      return "People";
    case "agents":
      return "Agents";
    case "messages":
      return "Most relevant";
    case "actions":
      return "Actions";
  }
}

function groupSearchResults(results: SearchResult[]): SearchResultSection[] {
  const resultsBySection = new Map<SearchResultSectionKey, SearchResult[]>();

  for (const result of results) {
    const sectionKey = getResultSectionKey(result);
    const sectionResults = resultsBySection.get(sectionKey) ?? [];
    sectionResults.push(result);
    resultsBySection.set(sectionKey, sectionResults);
  }

  return SEARCH_RESULT_SECTION_ORDER.flatMap((sectionKey) => {
    const sectionResults = resultsBySection.get(sectionKey);

    if (!sectionResults || sectionResults.length === 0) {
      return [];
    }

    return [
      {
        key: sectionKey,
        results: sectionResults,
        title: getSectionTitle(sectionKey),
      },
    ];
  });
}

function getSuggestedSearchResults(channels: Channel[]) {
  return channels
    .filter(
      (channel) =>
        !channel.archivedAt &&
        (channel.isMember || channel.channelType === "dm"),
    )
    .sort((a, b) => {
      const activityDiff =
        getChannelActivityTime(b) - getChannelActivityTime(a);
      if (activityDiff !== 0) {
        return activityDiff;
      }

      const typeRank = (channel: Channel) =>
        channel.channelType === "dm"
          ? 0
          : channel.channelType === "stream"
            ? 1
            : 2;
      const rankDiff = typeRank(a) - typeRank(b);
      if (rankDiff !== 0) {
        return rankDiff;
      }

      return a.name.localeCompare(b.name);
    })
    .slice(0, MAX_SEARCH_SUGGESTIONS)
    .map((channel) => ({
      kind: "channel" as const,
      channel,
    }));
}

const searchSkeletonRows = [
  {
    iconShape: "rounded-md",
    key: "channel",
    metaWidth: "w-16",
    previewWidth: "w-48",
    titleWidth: "w-28",
    trailingWidth: "w-14",
  },
  {
    iconShape: "rounded-full",
    key: "message",
    metaWidth: "w-24",
    previewWidth: "w-72",
    titleWidth: "w-24",
    trailingWidth: "w-20",
  },
  {
    iconShape: "rounded-full",
    key: "note",
    metaWidth: "w-20",
    previewWidth: "w-60",
    titleWidth: "w-32",
    trailingWidth: "w-16",
  },
] as const;

function SearchResultsSkeleton() {
  return (
    <div
      aria-hidden="true"
      className="p-1"
      data-testid="search-results-loading"
    >
      {searchSkeletonRows.map((row) => (
        <div
          className="flex w-full items-center gap-3 rounded-lg px-3 py-2"
          key={row.key}
        >
          <Skeleton className={cn("h-7 w-7 shrink-0", row.iconShape)} />
          <div className="min-w-0 flex-1">
            <div className="flex min-w-0 items-center gap-1.5">
              <Skeleton className={cn("h-4", row.titleWidth)} />
              <Skeleton className={cn("h-3", row.metaWidth)} />
            </div>
            <Skeleton
              className={cn("mt-1.5 h-3 max-w-full", row.previewWidth)}
            />
          </div>
          <Skeleton className={cn("h-3 shrink-0", row.trailingWidth)} />
        </div>
      ))}
    </div>
  );
}

export function TopbarSearch({
  channelLabels,
  channels,
  className,
  currentChannelId,
  currentPubkey,
  focusRequest = 0,
  onOpenChannel,
  onOpenResult,
  onOpenUser,
  onBrowseChannels,
  onCreateAgent,
  onCreateChannel,
  scopeFocusRequest = 0,
  suggestionChannels,
  variant = "bar",
}: TopbarSearchProps) {
  const apps = useAppsQuery().data;
  const relaySelfPubkey = useRelaySelfQuery().data;
  const [isOpen, setIsOpen] = React.useState(false);
  const [scopeChannelId, setScopeChannelId] = React.useState<string | null>(
    null,
  );
  const [selectedMenuIndex, setSelectedMenuIndex] = React.useState(0);
  const triggerRef = React.useRef<HTMLButtonElement>(null);
  const dialogInputRef = React.useRef<HTMLInputElement>(null);
  const { cancelDeferredModalOpen, openAfterExit, openNextFrame } =
    useDeferredModalOpen();
  const {
    channelLookup,
    debouncedQuery,
    fuzzyUserCandidatesQuery,
    isWaitingOnFromResolution,
    query,
    resultProfiles,
    results,
    searchQuery,
    setQuery,
    userSearchQuery,
  } = useSearchResults({
    channelLabels,
    channels,
    enabled: isOpen,
    limit: SEARCH_RESULT_LIMIT,
    scopeChannelId,
  });
  const trimmedQuery = query.trim();
  const isIconVariant = variant === "icon";
  const currentChannel = currentChannelId
    ? (channelLookup.get(currentChannelId) ?? null)
    : null;
  const scopeChannel = scopeChannelId
    ? (channelLookup.get(scopeChannelId) ?? null)
    : null;
  const scopeLabel = scopeChannel
    ? getChannelScopeLabel(scopeChannel, channelLabels, currentPubkey)
    : null;
  const currentPubkeyNormalized =
    currentPubkey && normalizePubkey(currentPubkey);
  const hasScopeAction = Boolean(currentChannel && !scopeChannel);
  const suggestedResults = React.useMemo(
    () => getSuggestedSearchResults(suggestionChannels ?? channels),
    [channels, suggestionChannels],
  );
  const suggestionActionResults = React.useMemo(() => {
    const actions: SearchResult[] = [];

    if (onBrowseChannels) {
      actions.push({
        kind: "action",
        action: {
          id: "browse-channels",
          title: "Browse channels",
        },
      });
    }

    if (onCreateChannel) {
      actions.push({
        kind: "action",
        action: {
          id: "create-channel",
          title: "Create a new channel",
        },
      });
    }

    if (onCreateAgent) {
      actions.push({
        kind: "action",
        action: {
          id: "create-agent",
          title: "Create a new agent",
        },
      });
    }

    return actions;
  }, [onBrowseChannels, onCreateAgent, onCreateChannel]);
  const suggestionResults = React.useMemo(
    () => [...suggestedResults, ...suggestionActionResults],
    [suggestedResults, suggestionActionResults],
  );
  const minimumQueryLength = getMinimumSearchQueryLength(scopeChannelId);
  const isShowingSuggestions =
    Math.max(debouncedQuery.length, trimmedQuery.length) < minimumQueryLength;
  const searchableResults = React.useMemo(
    () =>
      results.filter(
        (result) =>
          result.kind !== "user" ||
          normalizePubkey(result.user.pubkey) !== currentPubkeyNormalized,
      ),
    [currentPubkeyNormalized, results],
  );
  const searchResultSections = React.useMemo(
    () => groupSearchResults(searchableResults),
    [searchableResults],
  );
  const groupedSearchResults = React.useMemo(
    () => searchResultSections.flatMap((section) => section.results),
    [searchResultSections],
  );
  const activeResults = isShowingSuggestions
    ? scopeChannel
      ? []
      : suggestionResults
    : groupedSearchResults;
  const isSearchLoading =
    isWaitingOnFromResolution ||
    searchQuery.isLoading ||
    fuzzyUserCandidatesQuery.isLoading ||
    userSearchQuery.isLoading;

  const openSearchDialog = React.useCallback(
    (nextScopeChannelId: string | null = null) => {
      setScopeChannelId(nextScopeChannelId);
      setSelectedMenuIndex(0);
      openNextFrame(() => setIsOpen(true));
    },
    [openNextFrame],
  );

  const handleSearchOpenChange = React.useCallback(
    (nextOpen: boolean) => {
      if (nextOpen) {
        openSearchDialog(null);
        return;
      }

      cancelDeferredModalOpen();
      setSelectedMenuIndex(0);
      setScopeChannelId(null);
      setIsOpen(false);
    },
    [cancelDeferredModalOpen, openSearchDialog],
  );

  const openResult = React.useCallback(
    (result: SearchResult) => {
      setIsOpen(false);
      setScopeChannelId(null);
      setQuery("");

      if (result.kind === "channel") {
        onOpenChannel(result.channel.id);
        return;
      }

      if (result.kind === "user") {
        void onOpenUser?.(result.user);
        return;
      }

      if (result.kind === "action") {
        setSelectedMenuIndex(0);
        if (result.action.id === "browse-channels") {
          openAfterExit(() => {
            void onBrowseChannels?.();
          });
        } else if (result.action.id === "create-channel") {
          openAfterExit(() => {
            void onCreateChannel?.();
          });
        } else {
          openAfterExit(() => {
            void onCreateAgent?.();
          });
        }
        return;
      }

      onOpenResult(result.hit);
    },
    [
      onBrowseChannels,
      onCreateAgent,
      onCreateChannel,
      onOpenChannel,
      onOpenResult,
      onOpenUser,
      openAfterExit,
      setQuery,
    ],
  );

  // Edge-trigger: the counter never resets, so `!== 0` would replay on remount.
  const lastFocusRequestRef = React.useRef(focusRequest);
  React.useEffect(() => {
    if (focusRequest === lastFocusRequestRef.current) {
      return;
    }
    lastFocusRequestRef.current = focusRequest;

    openSearchDialog(null);
  }, [focusRequest, openSearchDialog]);

  const lastScopeFocusRequestRef = React.useRef(scopeFocusRequest);
  React.useEffect(() => {
    if (scopeFocusRequest === lastScopeFocusRequestRef.current) {
      return;
    }
    lastScopeFocusRequestRef.current = scopeFocusRequest;

    if (currentChannelId) {
      openSearchDialog(currentChannelId);
    }
  }, [currentChannelId, openSearchDialog, scopeFocusRequest]);

  const focusDialogInput = React.useCallback(() => {
    window.requestAnimationFrame(() => dialogInputRef.current?.focus());
  }, []);

  const activateCurrentChannelScope = React.useCallback(() => {
    if (!currentChannel) return;
    setScopeChannelId(currentChannel.id);
    setSelectedMenuIndex(0);
    focusDialogInput();
  }, [currentChannel, focusDialogInput]);

  const removeChannelScope = React.useCallback(() => {
    setScopeChannelId(null);
    setSelectedMenuIndex(0);
    focusDialogInput();
  }, [focusDialogInput]);

  React.useEffect(() => {
    if (!isOpen) {
      return;
    }

    const animationFrame = window.requestAnimationFrame(() => {
      dialogInputRef.current?.focus();
    });

    return () => {
      window.cancelAnimationFrame(animationFrame);
    };
  }, [isOpen]);

  const handleDialogInputKeyDown = useSearchMenuKeyboardNavigation({
    activeResults,
    hasLeadingAction: hasScopeAction,
    onActivateLeadingAction: activateCurrentChannelScope,
    onOpenResult: openResult,
    onRemoveScope: removeChannelScope,
    query,
    scopeActive: Boolean(scopeChannel),
    selectedMenuIndex,
    setSelectedMenuIndex,
  });

  const renderSearchResultRow = (result: SearchResult, index: number) => {
    const menuIndex = index + (hasScopeAction ? 1 : 0);
    if (result.kind === "message") {
      return (
        <MessageSearchResultRow
          apps={apps}
          channelLabels={channelLabels}
          channelLookup={channelLookup}
          currentPubkey={currentPubkey}
          hit={result.hit}
          key={resultKey(result)}
          menuIndex={menuIndex}
          onClick={() => openResult(result)}
          onMouseEnter={() => setSelectedMenuIndex(menuIndex)}
          relaySelfPubkey={relaySelfPubkey}
          resultProfiles={resultProfiles}
          selected={menuIndex === selectedMenuIndex}
        />
      );
    }
    const channelDisplayName =
      result.kind === "channel"
        ? getChannelDisplayName(result.channel, channelLabels)
        : null;
    const userDisplayName =
      result.kind === "user" ? getUserDisplayName(result.user) : null;
    const title =
      result.kind === "channel"
        ? channelDisplayName
        : result.kind === "action"
          ? result.action.title
          : userDisplayName;
    const preview =
      result.kind === "channel"
        ? getChannelPreview(result.channel)
        : result.kind === "action"
          ? result.action.description
          : getUserSecondaryLabel(result.user);
    const trailingLabel =
      result.kind === "channel"
        ? getChannelSuggestionMeta(result.channel)
        : null;

    return (
      <button
        aria-selected={menuIndex === selectedMenuIndex}
        className={cn(
          "search-result-row flex w-full gap-3 rounded-lg px-2.5 text-left transition-colors",
          "items-center",
          "py-2.5",
          menuIndex === selectedMenuIndex
            ? "bg-muted/45 text-foreground"
            : "hover:bg-muted/35",
        )}
        key={resultKey(result)}
        onClick={() => openResult(result)}
        onMouseEnter={() => setSelectedMenuIndex(menuIndex)}
        role="option"
        type="button"
        data-testid={resultTestId(result)}
        data-search-result-index={menuIndex}
      >
        {result.kind === "user" ? (
          <UserAvatar
            avatarUrl={result.user.avatarUrl}
            className="h-7 w-7"
            displayName={userDisplayName ?? result.user.pubkey}
            size="sm"
          />
        ) : (
          <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-background/70 text-muted-foreground">
            {React.createElement(resultIcon(result, channelLookup), {
              className: "h-4 w-4",
            })}
          </span>
        )}
        <span className="min-w-0 flex-1">
          <span className="block space-y-0.5">
            <span className="block truncate text-sm font-semibold">
              {title}
            </span>
            {preview ? (
              <span className="block truncate text-xs text-muted-foreground">
                {preview}
              </span>
            ) : null}
          </span>
        </span>
        {trailingLabel ? (
          <span className="shrink-0 text-2xs text-muted-foreground/75">
            {trailingLabel}
          </span>
        ) : null}
      </button>
    );
  };

  const renderSearchResultSections = (sections: SearchResultSection[]) => {
    let resultIndex = 0;

    return sections.map((section) => (
      <div data-search-section={section.key} key={section.key}>
        <div className={SEARCH_SECTION_TITLE_CLASS}>{section.title}</div>
        {section.results.map((result) =>
          renderSearchResultRow(result, resultIndex++),
        )}
      </div>
    ));
  };
  const currentChannelSearchAction =
    currentChannel && !scopeChannel ? (
      <CurrentChannelSearchAction
        channelLabel={getChannelScopeLabel(
          currentChannel,
          channelLabels,
          currentPubkey,
        )}
        channelType={currentChannel.channelType}
        isSelected={selectedMenuIndex === 0}
        onActivate={activateCurrentChannelScope}
        onMouseEnter={() => setSelectedMenuIndex(0)}
      />
    ) : null;
  const searchResultContent = isShowingSuggestions ? (
    scopeChannel ? null : suggestionResults.length === 0 ? (
      <div className="max-h-96 overflow-y-auto">
        {currentChannelSearchAction}
        <div
          className={cn(
            "px-4 text-sm text-muted-foreground",
            currentChannelSearchAction ? "pb-5" : "py-5",
          )}
        >
          <p>No recent activity yet.</p>
        </div>
      </div>
    ) : (
      <div
        aria-label="Recent activity"
        className="max-h-96 overflow-y-auto"
        role="listbox"
      >
        {currentChannelSearchAction}
        <div className="p-1.5">
          {(() => {
            let resultIndex = 0;

            return (
              <>
                {suggestedResults.length > 0 ? (
                  <div>
                    <div className={SEARCH_SECTION_TITLE_CLASS}>
                      Recent activity
                    </div>
                    {suggestedResults.map((result) =>
                      renderSearchResultRow(result, resultIndex++),
                    )}
                  </div>
                ) : null}
                {suggestionActionResults.length > 0 ? (
                  <div>
                    <div className={SEARCH_SECTION_TITLE_CLASS}>Actions</div>
                    {suggestionActionResults.map((result) =>
                      renderSearchResultRow(result, resultIndex++),
                    )}
                  </div>
                ) : null}
              </>
            );
          })()}
        </div>
      </div>
    )
  ) : isSearchLoading && searchableResults.length === 0 ? (
    <div className="max-h-[min(60vh,32rem)] overflow-y-auto">
      {currentChannelSearchAction}
      <SearchResultsSkeleton />
    </div>
  ) : searchQuery.error instanceof Error && searchableResults.length === 0 ? (
    <div className="max-h-[min(60vh,32rem)] overflow-y-auto">
      {currentChannelSearchAction}
      <p
        className={cn(
          "px-4 text-sm text-destructive",
          currentChannelSearchAction ? "pb-5" : "py-5",
        )}
      >
        {searchQuery.error.message}
      </p>
    </div>
  ) : searchableResults.length === 0 ? (
    <div className="max-h-[min(60vh,32rem)] overflow-y-auto">
      {currentChannelSearchAction}
      <p
        className={cn(
          "px-4 text-sm text-muted-foreground",
          currentChannelSearchAction ? "pb-5" : "py-5",
        )}
      >
        No {scopeChannel ? "messages" : "matches"} for{" "}
        <span className="font-semibold">{trimmedQuery}</span>
        {scopeLabel ? (
          <>
            {" "}
            in <span className="font-semibold">{scopeLabel}</span>
          </>
        ) : null}
        .
      </p>
    </div>
  ) : (
    <div
      className="max-h-[min(60vh,32rem)] overflow-y-auto"
      data-testid="search-results-list"
      role="listbox"
    >
      {currentChannelSearchAction}
      <div className="p-1.5">
        {renderSearchResultSections(searchResultSections)}
      </div>
    </div>
  );
  return (
    <div className={cn("relative", className)}>
      <Dialog open={isOpen} onOpenChange={handleSearchOpenChange}>
        <button
          aria-label="Search everything"
          className={
            isIconVariant
              ? "group/search flex size-6 items-center justify-center rounded p-1 text-sidebar-foreground/50 transition-colors hover:bg-sidebar-border/35 hover:text-sidebar-foreground focus-visible:bg-sidebar-border/35 focus-visible:text-sidebar-foreground focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-sidebar-ring"
              : "group/search flex h-8 w-full items-center gap-2 rounded-md bg-sidebar-border/35 px-2 text-left text-sm text-sidebar-foreground/55 transition-colors duration-150 ease-out hover:bg-sidebar-border/35 hover:text-sidebar-foreground focus-visible:bg-sidebar-border/35 focus-visible:text-sidebar-foreground focus-visible:outline-hidden focus-visible:ring-1 focus-visible:ring-sidebar-ring"
          }
          data-testid="open-search"
          onClick={() => openSearchDialog(null)}
          ref={triggerRef}
          title="Search everything"
          type="button"
        >
          <Search
            className={
              isIconVariant
                ? "h-4 w-4 shrink-0"
                : "h-4 w-4 shrink-0 text-sidebar-foreground/45 transition-colors duration-150 ease-out group-hover/search:text-sidebar-foreground/65 group-focus-visible/search:text-sidebar-foreground"
            }
          />
          {isIconVariant ? null : (
            <>
              <span
                className={cn(
                  "min-w-0 flex-1 truncate transition-colors duration-150 ease-out",
                  query
                    ? "text-sidebar-foreground"
                    : "text-sidebar-foreground/55",
                )}
              >
                {query || "Search everything"}
              </span>
              <kbd className="shrink-0 text-2xs text-sidebar-foreground/45">
                &#x2318;K
              </kbd>
            </>
          )}
        </button>
        <DialogContent
          aria-busy={isSearchLoading && searchableResults.length === 0}
          className="mt-[18vh] max-w-2xl self-start gap-0 overflow-hidden rounded-2xl p-0 shadow-2xl"
          data-testid="search-results"
          onOpenAutoFocus={(event) => {
            event.preventDefault();
            dialogInputRef.current?.focus();
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            triggerRef.current?.focus();
          }}
          showCloseButton={false}
        >
          <DialogTitle className="sr-only">
            {scopeLabel ? `Search in ${scopeLabel}` : "Search everything"}
          </DialogTitle>
          <SearchDialogInputRow
            inputRef={dialogInputRef}
            onChange={(nextQuery) => {
              setQuery(nextQuery);
              setSelectedMenuIndex(0);
            }}
            onKeyDown={handleDialogInputKeyDown}
            onRemoveScope={removeChannelScope}
            query={query}
            scopeLabel={scopeLabel}
          />
          {searchResultContent}
        </DialogContent>
      </Dialog>
    </div>
  );
}
