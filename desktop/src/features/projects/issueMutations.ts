import { useMutation, useQueryClient } from "@tanstack/react-query";

import { createGithubIssue } from "@/shared/api/projectGit";
import { relayClient } from "@/shared/api/relayClient";
import { signRelayEvent } from "@/shared/api/tauri";
import { KIND_GIT_ISSUE } from "@/shared/constants/kinds";
import { isGitHubCloneUrl } from "@/features/projects/lib/projectGitError";
import { githubIssueId } from "@/features/projects/lib/projectGithubIssues";
import { projectIssueWriteInvalidationKeys } from "@/features/projects/lib/projectGithubIssueWrites";
import type { Repository as Project } from "./hooks";
import { buildGitIssueTags } from "./projectIssues.mjs";
import type { ProjectTaskCategory } from "./projectTaskCategories";

type CreateProjectIssueInput = {
  title: string;
  body: string;
  category?: ProjectTaskCategory;
};

export async function publishProjectIssue(
  project: Project,
  input: CreateProjectIssueInput,
) {
  const event = await signRelayEvent({
    kind: KIND_GIT_ISSUE,
    content: input.body.trim(),
    tags: buildGitIssueTags({
      repoAddress: project.repoAddress,
      repoOwner: project.owner,
      title: input.title,
      labels: [input.category ?? "issue"],
    }),
  });
  await relayClient.publishEvent(
    event,
    "Timed out creating task.",
    "Failed to create task.",
  );
  return event.id;
}

/** Create an issue through exactly one repository-native backend. */
export async function createProjectIssueWith(
  project: Project,
  input: CreateProjectIssueInput,
  loaders: {
    createGithub: (input: {
      cloneUrl: string;
      title: string;
      body: string;
    }) => Promise<{ number: number }>;
    publishBuzz: typeof publishProjectIssue;
  },
): Promise<string> {
  const cloneUrl = project.cloneUrls[0] ?? "";
  if (isGitHubCloneUrl(cloneUrl)) {
    const issue = await loaders.createGithub({
      cloneUrl,
      title: input.title,
      body: input.body,
    });
    return githubIssueId(issue.number);
  }
  return loaders.publishBuzz(project, input);
}

/** Query keys invalidated after a repository-native issue create. */
export function projectIssueInvalidationKeys(
  project: Pick<Project, "id" | "cloneUrls">,
): readonly unknown[][] {
  return projectIssueWriteInvalidationKeys(project);
}

export function useCreateProjectIssueMutation(
  project: Project | null | undefined,
) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectIssueInput) => {
      if (!project) throw new Error("No project selected.");
      return createProjectIssueWith(project, input, {
        createGithub: createGithubIssue,
        publishBuzz: publishProjectIssue,
      });
    },
    onSuccess: async () => {
      if (!project) return;
      await Promise.all(
        projectIssueInvalidationKeys(project).map((queryKey) =>
          queryClient.invalidateQueries({ queryKey }),
        ),
      );
    },
  });
}
