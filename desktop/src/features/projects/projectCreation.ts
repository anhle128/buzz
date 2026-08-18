import {
  KIND_PROJECT_ANNOUNCEMENT,
  KIND_REPO_ANNOUNCEMENT,
} from "@/shared/constants/kinds";
import { isValidProjectChannelId } from "./projectModels";

export type ProjectEventTemplate = {
  kind: number;
  content: string;
  tags: string[][];
};

export function isUnsupportedProjectKindError(error: unknown): boolean {
  return (
    error instanceof Error &&
    /(?:unknown|unsupported) event kind/i.test(error.message)
  );
}

/** Derives the ASCII NIP-34 repository d-tag from a user-facing name. */
export function repositoryDtagFromName(name: string): string {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function projectDtagFromName(name: string): string {
  return repositoryDtagFromName(name);
}

export type InitialProjectEventTemplates = {
  /** Project d-tag derived from the Create project Name field. */
  dtag: string;
  /** Initial repository d-tag derived from Repository name, or `dtag` when omitted. */
  repositoryDtag: string;
  project: ProjectEventTemplate;
  repository: ProjectEventTemplate;
  repositoryAddress: string;
};

export function buildInitialProjectEventTemplates({
  accessChannelId,
  cloneUrl,
  description,
  name,
  ownerPubkey,
  repositoryName,
  webUrl,
}: {
  accessChannelId: string;
  cloneUrl?: string;
  description?: string;
  name: string;
  ownerPubkey: string;
  repositoryName?: string;
  webUrl?: string;
}): InitialProjectEventTemplates {
  const normalizedName = name.trim();
  if (!normalizedName) {
    throw new Error("Project name is required.");
  }
  if (new TextEncoder().encode(normalizedName).byteLength > 256) {
    throw new Error("Project name must not exceed 256 bytes.");
  }
  const dtag = projectDtagFromName(normalizedName);
  if (!dtag) {
    throw new Error("Project name must include letters or numbers.");
  }

  const normalizedRepositoryName = repositoryName?.trim() ?? "";
  let repositoryDisplayName = normalizedName;
  let repositoryDtag = dtag;
  if (normalizedRepositoryName) {
    if (new TextEncoder().encode(normalizedRepositoryName).byteLength > 256) {
      throw new Error("Repository name must not exceed 256 bytes.");
    }
    repositoryDtag = repositoryDtagFromName(normalizedRepositoryName);
    if (!repositoryDtag) {
      throw new Error("Repository name must include letters or numbers.");
    }
    if (repositoryDtag.length > 64) {
      throw new Error("Repository name slug must not exceed 64 characters.");
    }
    repositoryDisplayName = normalizedRepositoryName;
  }

  const normalizedOwner = ownerPubkey.trim().toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(normalizedOwner)) {
    throw new Error("Project owner public key is invalid.");
  }

  const normalizedDescription = description?.trim() ?? "";
  if (new TextEncoder().encode(normalizedDescription).byteLength > 2_048) {
    throw new Error("Project description must not exceed 2,048 bytes.");
  }
  const repositoryTags: string[][] = [
    ["d", repositoryDtag],
    ["name", repositoryDisplayName],
  ];
  const projectTags: string[][] = [
    ["d", dtag],
    ["name", normalizedName],
  ];
  const normalizedAccessChannelId = accessChannelId.trim();
  if (!isValidProjectChannelId(normalizedAccessChannelId)) {
    throw new Error("Repository access channel is invalid.");
  }
  repositoryTags.push(["buzz-channel", normalizedAccessChannelId]);
  projectTags.push(["buzz-channel", normalizedAccessChannelId]);
  if (normalizedDescription) {
    repositoryTags.push(["description", normalizedDescription]);
    projectTags.push(["description", normalizedDescription]);
  }
  const normalizedCloneUrl = cloneUrl?.trim();
  if (normalizedCloneUrl) {
    repositoryTags.push(["clone", normalizedCloneUrl]);
  }
  const normalizedWebUrl = webUrl?.trim();
  if (normalizedWebUrl) {
    repositoryTags.push(["web", normalizedWebUrl]);
  }

  const repositoryAddress = `${KIND_REPO_ANNOUNCEMENT}:${normalizedOwner}:${repositoryDtag}`;
  projectTags.push(["a", repositoryAddress]);

  return {
    dtag,
    repositoryDtag,
    project: {
      kind: KIND_PROJECT_ANNOUNCEMENT,
      content: "",
      tags: projectTags,
    },
    repository: {
      kind: KIND_REPO_ANNOUNCEMENT,
      content: normalizedDescription,
      tags: repositoryTags,
    },
    repositoryAddress,
  };
}
