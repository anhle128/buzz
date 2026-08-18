import * as React from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";

import {
  fetchProjects,
  type Project,
  projectsQueryKey,
} from "@/features/projects/hooks";
import {
  buildInitialProjectEventTemplates,
  isUnsupportedProjectKindError,
} from "@/features/projects/projectCreation";
import { buildProjectReadModels } from "@/features/projects/projectModels";
import { relayClient } from "@/shared/api/relayClient";
import type { RelayEvent } from "@/shared/api/types";
import { KIND_REPO_ANNOUNCEMENT } from "@/shared/constants/kinds";
import { getCachedRelayOrigin } from "@/shared/lib/mediaUrl";
import { signRelayEvent } from "@/shared/api/tauri";
import { getIdentity } from "@/shared/api/tauriIdentity";

export type CreateProjectInput = {
  accessChannelId: string;
  name: string;
  description?: string;
  cloneUrl?: string;
  webUrl?: string;
  repositoryName?: string;
};

export type CreateProjectResult = {
  project: Project;
  compatibilityWarning?: string;
};

type FetchEventsInput = Parameters<(typeof relayClient)["fetchEvents"]>[0];

/** Test seams for createProject relay, signing, and project-list I/O. */
export type CreateProjectDeps = {
  fetchEvents: (filter: FetchEventsInput) => Promise<RelayEvent[]>;
  fetchProjects: typeof fetchProjects;
  getIdentity: () => Promise<{ pubkey: string }>;
  publishEvent: (
    event: RelayEvent,
    timeoutMessage: string,
    sendErrorMessage: string,
  ) => Promise<unknown>;
  signRelayEvent: typeof signRelayEvent;
};

type CreateProjectResume = {
  repositoryAddress: string;
  repositoryEventId: string;
};

/** Publishes a project announcement and its initial NIP-34 repository. */
export async function createProject(
  input: CreateProjectInput,
  resumableProjects: Map<string, CreateProjectResume>,
  deps: Partial<CreateProjectDeps> = {},
): Promise<CreateProjectResult> {
  const {
    fetchEvents = relayClient.fetchEvents.bind(relayClient),
    fetchProjects: fetchProjectsFn = fetchProjects,
    getIdentity: getIdentityFn = getIdentity,
    publishEvent = relayClient.publishEvent.bind(relayClient),
    signRelayEvent: signRelayEventFn = signRelayEvent,
  } = deps;

  const identity = await getIdentityFn();
  const templates = buildInitialProjectEventTemplates({
    ...input,
    ownerPubkey: identity.pubkey,
  });
  const existing = await fetchProjectsFn();
  const ownerPubkey = identity.pubkey.toLowerCase();
  const existingProject = existing.find(
    (project) =>
      project.owner.toLowerCase() === ownerPubkey &&
      project.dtag === templates.dtag,
  );
  const projectId = `${ownerPubkey}:${templates.dtag}`;
  const resume = resumableProjects.get(projectId);
  const canResume = resume?.repositoryAddress === templates.repositoryAddress;
  if (existingProject && !canResume) {
    throw new Error(`You already have a project named "${templates.dtag}".`);
  }
  if (existingProject && !existingProject.legacy) {
    if (
      existingProject.repositories.some(
        (repository) => repository.repoAddress === templates.repositoryAddress,
      )
    ) {
      resumableProjects.delete(projectId);
      return { project: existingProject };
    }
    throw new Error(`You already have a project named "${templates.dtag}".`);
  }

  let repositoryEvent: RelayEvent | null = null;
  if (!existingProject && templates.repositoryDtag !== templates.dtag) {
    const existingRepoHeads = await fetchEvents({
      kinds: [KIND_REPO_ANNOUNCEMENT],
      authors: [ownerPubkey],
      "#d": [templates.repositoryDtag],
      limit: 1,
    });
    if (existingRepoHeads.length > 0) {
      if (
        !canResume ||
        existingRepoHeads[0]?.id !== resume?.repositoryEventId
      ) {
        throw new Error(
          `A repository named "${templates.repositoryDtag}" already exists (as a standalone repository or in another project). Choose a different name to avoid overwriting it.`,
        );
      }
      repositoryEvent = existingRepoHeads[0] ?? null;
    }
  }

  const projectEvent = await signRelayEventFn(templates.project);

  if (!existingProject && !repositoryEvent) {
    repositoryEvent = await signRelayEventFn(templates.repository);
    resumableProjects.set(projectId, {
      repositoryAddress: templates.repositoryAddress,
      repositoryEventId: repositoryEvent.id,
    });
    await publishEvent(
      repositoryEvent,
      "Timed out creating the initial repository.",
      "Failed to create the initial repository.",
    );
  }

  try {
    await publishEvent(
      projectEvent,
      "Timed out creating project.",
      "Failed to create project.",
    );
  } catch (error) {
    if (!isUnsupportedProjectKindError(error)) throw error;

    const [legacyProject] = existingProject?.legacy
      ? [existingProject]
      : buildProjectReadModels({
          projectEvents: [],
          repositoryEvents: repositoryEvent ? [repositoryEvent] : [],
          relayOrigin: getCachedRelayOrigin(),
        });
    if (!legacyProject) throw error;

    resumableProjects.delete(projectId);
    return {
      project: legacyProject,
      compatibilityWarning:
        "The repository was created, but this relay does not support multi-repository projects yet. It will appear as a standalone project.",
    };
  }

  const [project] = repositoryEvent
    ? buildProjectReadModels({
        projectEvents: [projectEvent],
        repositoryEvents: [repositoryEvent],
        relayOrigin: getCachedRelayOrigin(),
      })
    : (await fetchProjectsFn()).filter(
        (candidate) =>
          candidate.owner.toLowerCase() === ownerPubkey &&
          candidate.dtag === templates.dtag &&
          !candidate.legacy,
      );
  if (!project) {
    throw new Error("The project was created but could not be read.");
  }
  resumableProjects.delete(projectId);
  return { project };
}

/** Mutation that creates a project and inserts it into the projects cache. */
export function useCreateProjectMutation() {
  const queryClient = useQueryClient();
  const resumableProjectsRef = React.useRef(
    new Map<string, CreateProjectResume>(),
  );

  return useMutation({
    mutationFn: (input: CreateProjectInput) =>
      createProject(input, resumableProjectsRef.current),
    onSuccess: ({ project }) => {
      queryClient.setQueryData<Project[]>(projectsQueryKey, (current = []) => [
        project,
        ...current.filter(
          (candidate) =>
            candidate.id !== project.id &&
            !(
              candidate.legacy &&
              candidate.owner === project.owner &&
              candidate.dtag === project.dtag
            ),
        ),
      ]);
      void queryClient.invalidateQueries({ queryKey: projectsQueryKey });
    },
  });
}
