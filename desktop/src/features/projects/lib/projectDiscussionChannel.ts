import type { DiscussionChannel } from "@/features/projects/lib/discussionChannels";
import { isValidProjectChannelId } from "@/features/projects/projectModels";
import type { Channel } from "@/shared/api/types";

/** Viewer-resolvable channel used by project discussion routing. */
export type ProjectDiscussionChannel = {
  id: string;
  name: string;
};

type ResolvableChannel = Pick<
  Channel,
  "id" | "name" | "archivedAt" | "channelType"
>;

function isUsableDiscussionChannel(channel: ResolvableChannel): boolean {
  return channel.archivedAt === null && channel.channelType !== "dm";
}

/**
 * Resolves the selected repository binding first and the project binding second.
 * Invalid or unreadable metadata is omitted instead of becoming a broken link.
 */
export function resolveProjectDiscussionChannel(input: {
  repositoryChannelId: string | null | undefined;
  projectChannelId: string | null | undefined;
  channels: readonly ResolvableChannel[];
}): ProjectDiscussionChannel | null {
  const candidates = [input.repositoryChannelId, input.projectChannelId];

  for (const candidate of candidates) {
    if (!candidate || !isValidProjectChannelId(candidate)) continue;
    const normalizedCandidate = candidate.toLowerCase();
    const channel = input.channels.find(
      (item) => item.id.toLowerCase() === normalizedCandidate,
    );
    if (!channel || !isUsableDiscussionChannel(channel)) continue;
    return { id: channel.id, name: channel.name };
  }

  return null;
}

/**
 * Pins the bound channel before FTS-discovered rows while preserving hit data.
 */
export function mergeBoundDiscussionChannel(
  bound: ProjectDiscussionChannel | null,
  discovered: readonly DiscussionChannel[],
): DiscussionChannel[] {
  if (!bound) return [...discovered];

  const existing = discovered.find((channel) => channel.id === bound.id);
  const remainder = discovered.filter((channel) => channel.id !== bound.id);
  if (existing) return [existing, ...remainder];

  return [
    {
      id: bound.id,
      name: bound.name,
      messageCount: 0,
      lastActivityAt: 0,
      participants: [],
    },
    ...remainder,
  ];
}
