import { AlertTriangle } from "lucide-react";

import type { TimelineMessage } from "@/features/messages/types";
import { UserProfilePopover } from "@/features/profile/ui/UserProfilePopover";
import { cn } from "@/shared/lib/cn";
import { Badge } from "@/shared/ui/badge";
import { UserAvatar } from "@/shared/ui/UserAvatar";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";
import { MessageAgentOwner } from "./MessageAgentOwner";
import {
  MessageAuthorText,
  MessageHeaderRow,
  MessageMetaSegments,
} from "./MessageHeader";
import { MessageTimestamp } from "./MessageTimestamp";

export function MessageAppBadge() {
  return (
    <Badge data-testid="message-app-badge" variant="secondary">
      App
    </Badge>
  );
}

export function renderMessageAuthorIdentity({
  hideAgentAccessBadge = false,
  isDisplayedAsContinuation,
  isThreadReplyLayout,
  message,
  profilePopoverRole,
}: {
  hideAgentAccessBadge?: boolean;
  isDisplayedAsContinuation: boolean;
  isThreadReplyLayout: boolean;
  message: TimelineMessage;
  profilePopoverRole?: string;
}) {
  const avatarButtonRadiusClass = "rounded-full";
  const interactiveAuthor = Boolean(message.pubkey) && !message.isApp;

  const showRespondToIndicator =
    !message.isApp &&
    (message.respondTo === "anyone" || message.respondTo === "allowlist");

  const avatarNode = (
    <div className="relative shrink-0">
      <UserAvatar
        accent={message.accent}
        avatarUrl={message.avatarUrl ?? null}
        className="shrink-0"
        displayName={message.author}
        testId="message-avatar"
      />
      {showRespondToIndicator &&
      !hideAgentAccessBadge &&
      !isThreadReplyLayout ? (
        <span
          className={cn(
            "absolute -bottom-0.5 -right-0.5 flex h-3 w-3 items-center justify-center rounded-full bg-background",
          )}
          role="img"
          aria-label={
            message.respondTo === "anyone"
              ? "Anyone can send instructions to this agent"
              : "Selected people can send instructions to this agent"
          }
          title={
            message.respondTo === "anyone"
              ? "Anyone can send instructions to this agent"
              : "Selected people can send instructions to this agent"
          }
        >
          {message.respondTo === "anyone" ? (
            <AlertTriangle
              aria-hidden="true"
              className="h-2.5 w-2.5 fill-background text-amber-500"
            />
          ) : (
            <span className="h-2 w-2 rounded-full bg-blue-500" />
          )}
        </span>
      ) : null}
    </div>
  );

  const continuationTimestampGutter = (
    <div
      aria-hidden="true"
      className={cn(
        "flex w-9 shrink-0 justify-end items-start pt-0.5",
        isThreadReplyLayout ? "self-start" : "self-stretch",
      )}
    >
      <MessageTimestamp
        className="opacity-0 transition-opacity group-hover/message:opacity-100 group-focus-within/message:opacity-100"
        createdAt={message.createdAt}
        hideDayPeriod
      />
    </div>
  );

  const avatarGutter = isDisplayedAsContinuation ? (
    continuationTimestampGutter
  ) : interactiveAuthor && message.pubkey ? (
    <UserProfilePopover
      pubkey={message.pubkey}
      role={profilePopoverRole}
      botIdenticonValue={message.author}
    >
      <button
        className={cn(
          "flex shrink-0 items-start focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring",
          avatarButtonRadiusClass,
        )}
        type="button"
      >
        {avatarNode}
      </button>
    </UserProfilePopover>
  ) : (
    <div className="flex shrink-0 items-start">{avatarNode}</div>
  );

  const authorNode = interactiveAuthor ? (
    <MessageAuthorText hoverUnderline>{message.author}</MessageAuthorText>
  ) : (
    <MessageAuthorText as="h3">{message.author}</MessageAuthorText>
  );
  const agentOwnerNode =
    message.isAgent && !message.isApp ? (
      <MessageAgentOwner
        ownerLabel={message.ownerLabel}
        ownerPubkey={message.ownerPubkey}
      />
    ) : null;
  const appBadgeNode = message.isApp ? <MessageAppBadge /> : null;

  const statusMetadataNode =
    message.pending || message.edited ? (
      <>
        {message.pending ? (
          <p
            className="font-normal text-muted-foreground/70"
            data-testid="message-send-status"
          >
            Sending…
          </p>
        ) : null}
        {message.edited ? (
          <Tooltip>
            <TooltipTrigger asChild>
              <p className="text-muted-foreground/70">(edited)</p>
            </TooltipTrigger>
            <TooltipContent>This message has been edited</TooltipContent>
          </Tooltip>
        ) : null}
      </>
    ) : null;

  const inlineMetadataNode = (
    <div className="flex shrink-0 items-baseline gap-2 text-xs">
      <MessageTimestamp createdAt={message.createdAt} />
      {statusMetadataNode}
    </div>
  );

  const personaNode =
    message.personaDisplayName &&
    message.personaDisplayName !== message.author ? (
      <span className="text-xs text-muted-foreground">
        {message.personaDisplayName}
      </span>
    ) : null;

  const continuationMetadata =
    isDisplayedAsContinuation && statusMetadataNode ? (
      <div className="mt-0.5 flex items-baseline gap-2 text-xs">
        {statusMetadataNode}
      </div>
    ) : null;

  const header = isDisplayedAsContinuation ? null : (
    <MessageHeaderRow>
      {interactiveAuthor && message.pubkey ? (
        <UserProfilePopover
          pubkey={message.pubkey}
          role={profilePopoverRole}
          botIdenticonValue={message.author}
        >
          <button
            className="truncate rounded leading-message-author focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring"
            type="button"
          >
            {authorNode}
          </button>
        </UserProfilePopover>
      ) : (
        authorNode
      )}
      {/* Author is not a segment: "Alice 9:53 AM" needs no divider. */}
      <MessageMetaSegments
        segments={[
          { key: "app", node: appBadgeNode },
          { key: "owner", node: agentOwnerNode },
          { key: "timestamp", node: inlineMetadataNode },
          { key: "persona", node: personaNode },
        ]}
      />
    </MessageHeaderRow>
  );

  return {
    avatarGutter,
    continuationMetadata,
    header,
  };
}
