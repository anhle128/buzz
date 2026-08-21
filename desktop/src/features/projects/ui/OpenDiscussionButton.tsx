import { Hash } from "lucide-react";

import type { ProjectDiscussionChannel } from "@/features/projects/lib/projectDiscussionChannel";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/shared/ui/tooltip";

/** Opens the resolved discussion channel from the project detail chrome. */
export function OpenDiscussionButton({
  channel,
  onOpen,
}: {
  channel: ProjectDiscussionChannel | null;
  onOpen: (channelId: string) => void;
}) {
  if (!channel) return null;

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          aria-label="Open Discussion"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted/40 hover:text-foreground focus-visible:outline-hidden focus-visible:ring-2 focus-visible:ring-ring"
          data-testid="project-open-discussion"
          onClick={() => onOpen(channel.id)}
          type="button"
        >
          <Hash className="h-3.5 w-3.5" />
        </button>
      </TooltipTrigger>
      <TooltipContent>{`Open #${channel.name}`}</TooltipContent>
    </Tooltip>
  );
}
