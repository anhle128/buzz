import * as React from "react";

import { useChannelsQuery } from "@/features/channels/hooks";
import type { Project, Repository } from "@/features/projects/hooks";
import {
  type ProjectDiscussionChannel,
  resolveProjectDiscussionChannel,
} from "@/features/projects/lib/projectDiscussionChannel";

/** Resolves the viewer-readable discussion channel for the active project repository. */
export function useProjectDiscussionChannel(
  project: Project | null | undefined,
  repository: Repository | null | undefined,
): ProjectDiscussionChannel | null {
  const channelsQuery = useChannelsQuery();
  return React.useMemo(
    () =>
      resolveProjectDiscussionChannel({
        repositoryChannelId: repository?.channelId,
        projectChannelId: project?.projectChannelId,
        channels: channelsQuery.data ?? [],
      }),
    [channelsQuery.data, project?.projectChannelId, repository?.channelId],
  );
}
