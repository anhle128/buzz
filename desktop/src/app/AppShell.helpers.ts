import { isThreadReply } from "@/features/messages/lib/threading";
import type { DesktopNotificationTarget } from "@/features/notifications/lib/desktop";
import type { SearchHit } from "@/shared/api/types";

export type AppView =
  | "home"
  | "channel"
  | "messages"
  | "agents"
  | "workflows"
  | "pulse"
  | "projects";

const WINDOW_DRAG_HANDLE_HEIGHT = 44;
const TAURI_DRAG_REGION_ATTR = "data-tauri-drag-region";
const WINDOW_DRAG_INTERACTIVE_SELECTOR =
  'button, a, input, textarea, select, label, summary, [role="button"], [role="link"], [role="menuitem"], [role="tab"], [role="checkbox"], [role="radio"], [role="switch"], [role="option"], [contenteditable="true"], [tabindex]:not([tabindex="-1"])';

const CLICKABLE_TAGS = new Set([
  "A",
  "BUTTON",
  "INPUT",
  "SELECT",
  "TEXTAREA",
  "LABEL",
  "SUMMARY",
]);
const INTERACTIVE_ROLES = new Set([
  "button",
  "link",
  "menuitem",
  "tab",
  "checkbox",
  "radio",
  "switch",
  "option",
]);

function isClickableElement(element: HTMLElement) {
  return (
    CLICKABLE_TAGS.has(element.tagName) ||
    (element.hasAttribute("contenteditable") &&
      element.getAttribute("contenteditable") !== "false") ||
    (element.hasAttribute("tabindex") &&
      element.getAttribute("tabindex") !== "-1") ||
    INTERACTIVE_ROLES.has(element.getAttribute("role") ?? "")
  );
}

function isTauriDragRegionEvent(event: MouseEvent | PointerEvent) {
  const path = event.composedPath();
  const directTarget = path[0];

  for (const item of path) {
    if (!(item instanceof HTMLElement)) continue;

    const attr = item.getAttribute(TAURI_DRAG_REGION_ATTR);

    if (isClickableElement(item) && attr === null) return false;
    if (attr === null) continue;
    if (attr === "false") return false;
    if (attr === "deep") return true;
    if (attr === "" || attr === "true") return item === directTarget;
  }

  return false;
}

export function isWindowDragHandleEvent(event: MouseEvent | PointerEvent) {
  if (isTauriDragRegionEvent(event)) {
    return true;
  }

  if (event.clientY > WINDOW_DRAG_HANDLE_HEIGHT) {
    return false;
  }

  const target = event.target;
  return !(
    target instanceof Element &&
    target.closest(WINDOW_DRAG_INTERACTIVE_SELECTOR)
  );
}

export function shouldBounceForChannelNotification(tags: string[][]): boolean {
  return !isThreadReply(tags);
}

export function markAllReadSources({
  activeChannelId,
  channelActivityItems,
  markAllChannelReadMarkers,
  markActiveChannelRead,
  undoUnreadFeedItem,
  unreadFeedItemIds,
}: {
  activeChannelId: string | null;
  channelActivityItems: ReadonlyArray<{
    channelId: string | null;
    createdAt: number;
  }>;
  markAllChannelReadMarkers: () => void;
  markActiveChannelRead: (channelId: string, createdAt: number) => void;
  undoUnreadFeedItem: (itemId: string) => void;
  unreadFeedItemIds: ReadonlySet<string>;
}) {
  for (const itemId of unreadFeedItemIds) {
    undoUnreadFeedItem(itemId);
  }
  markAllChannelReadMarkers();

  if (!activeChannelId) return;

  let latestActivityAt: number | null = null;
  for (const item of channelActivityItems) {
    if (item.channelId !== activeChannelId) continue;
    latestActivityAt = Math.max(latestActivityAt ?? 0, item.createdAt);
  }
  if (latestActivityAt !== null) {
    markActiveChannelRead(activeChannelId, latestActivityAt);
  }
}

export function toSearchHit(
  target: DesktopNotificationTarget,
): SearchHit | null {
  if (!target.eventId) {
    return null;
  }

  return {
    eventId: target.eventId,
    content: target.content ?? "",
    kind: target.kind ?? 9,
    pubkey: target.pubkey ?? "",
    channelId: target.channelId,
    channelName: target.channelName ?? null,
    createdAt: target.createdAt ?? Math.floor(Date.now() / 1_000),
    score: 0,
    threadRootId: target.threadRootId ?? null,
  };
}

/** Navigation action resolved from a desktop notification click target. */
export type DesktopNotificationAction =
  | { type: "agent-activity"; agentPubkey: string; channelId: string | null }
  | { type: "home" }
  | { type: "channel"; channelId: string }
  | { type: "search-hit"; hit: SearchHit };

/** Dependencies required to execute one desktop notification click. */
export type DesktopNotificationActionDependencies = {
  revealDesktopAppWindow: () => Promise<void>;
  openAgentActivity: (
    pubkey: string,
    options: { channelId: string | null },
  ) => unknown;
  goHome: () => Promise<unknown>;
  goChannel: (channelId: string) => Promise<unknown>;
  openSearchHit: (hit: SearchHit) => Promise<unknown>;
};

/** Resolve a desktop notification target to one navigation action. */
export function resolveDesktopNotificationAction(
  target: DesktopNotificationTarget,
): DesktopNotificationAction {
  if (target.agentPubkey) {
    return {
      type: "agent-activity",
      agentPubkey: target.agentPubkey,
      channelId: target.channelId,
    };
  }
  if (!target.channelId) {
    return { type: "home" };
  }
  const hit = toSearchHit(target);
  return hit
    ? { type: "search-hit", hit }
    : { type: "channel", channelId: target.channelId };
}

/** Reveal Buzz and execute exactly one resolved notification action. */
export async function executeDesktopNotificationAction(
  target: DesktopNotificationTarget,
  dependencies: DesktopNotificationActionDependencies,
): Promise<void> {
  await dependencies.revealDesktopAppWindow();
  const action = resolveDesktopNotificationAction(target);
  if (action.type === "agent-activity") {
    dependencies.openAgentActivity(action.agentPubkey, {
      channelId: action.channelId,
    });
    return;
  }
  if (action.type === "home") {
    await dependencies.goHome();
    return;
  }
  if (action.type === "channel") {
    await dependencies.goChannel(action.channelId);
    return;
  }
  await dependencies.openSearchHit(action.hit);
}

export function deriveShellRoute(pathname: string): {
  selectedChannelId: string | null;
  selectedView: AppView;
} {
  if (pathname.startsWith("/channels/")) {
    const [, , rawChannelId] = pathname.split("/");
    return {
      selectedChannelId: rawChannelId ? decodeURIComponent(rawChannelId) : null,
      selectedView: "channel",
    };
  }

  if (pathname === "/messages/new") {
    return {
      selectedChannelId: null,
      selectedView: "messages",
    };
  }

  if (pathname === "/agents") {
    return {
      selectedChannelId: null,
      selectedView: "agents",
    };
  }

  if (pathname === "/workflows" || pathname.startsWith("/workflows/")) {
    return {
      selectedChannelId: null,
      selectedView: "workflows",
    };
  }

  if (pathname === "/projects" || pathname.startsWith("/projects/")) {
    return {
      selectedChannelId: null,
      selectedView: "projects",
    };
  }

  if (pathname === "/pulse") {
    return {
      selectedChannelId: null,
      selectedView: "pulse",
    };
  }

  return {
    selectedChannelId: null,
    selectedView: "home",
  };
}
