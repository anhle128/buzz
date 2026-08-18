import * as React from "react";

export function useProjectRepositoryRefSelection(input: {
  branchOptions: string[];
  defaultBranch: string | null;
  projectAvailable: boolean;
  projectPending: boolean;
  tags: Array<{ name: string }>;
}) {
  const [selectedBranch, setSelectedBranch] = React.useState<string | null>(
    null,
  );
  const [selectedTag, setSelectedTag] = React.useState<string | null>(null);
  const activeBranch =
    selectedBranch ?? input.defaultBranch ?? input.branchOptions[0] ?? null;

  React.useEffect(() => {
    if (!input.projectAvailable) {
      if (input.projectPending) return;
      setSelectedBranch(null);
      setSelectedTag(null);
      return;
    }
    setSelectedBranch((currentBranch) => {
      if (currentBranch && input.branchOptions.includes(currentBranch)) {
        return currentBranch;
      }
      // Keep auto-selection derived from defaultBranch so a later GitHub HEAD
      // can replace an announced fallback that is still in the option list.
      return null;
    });
    setSelectedTag((currentTag) => {
      if (currentTag && input.tags.some((tag) => tag.name === currentTag)) {
        return currentTag;
      }
      return null;
    });
  }, [
    input.branchOptions,
    input.projectAvailable,
    input.projectPending,
    input.tags,
  ]);

  const selectBranch = React.useCallback((branch: string | null) => {
    setSelectedBranch(branch);
    setSelectedTag(null);
  }, []);
  const selectTag = React.useCallback((tag: string) => {
    setSelectedTag(tag);
  }, []);

  return {
    activeBranch,
    selectBranch,
    selectedTag,
    selectTag,
  };
}
