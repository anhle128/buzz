import { useMutation, useQueryClient } from "@tanstack/react-query";

import { appsQueryKey } from "@/features/apps/hooks/useAppsQuery";
import {
  type AppAdminCommand,
  type AppCredentials,
  buildCreateCommand,
  buildDisableCommand,
  buildEnableCommand,
  buildRotateSecretCommand,
  buildUpdateCommand,
  parseAppSecretAck,
  serializeAppCommand,
} from "@/features/apps/lib/appCommands";
import { useRelaySelfQuery } from "@/features/moderation/hooks";
import { relayClient } from "@/shared/api/relayClient";
import { getRelayHttpUrl, signRelayEvent } from "@/shared/api/tauri";
import { KIND_APP_ADMIN_COMMAND } from "@/shared/constants/kinds";

async function publishAppCommand(
  command: AppAdminCommand,
  timeoutMessage: string,
  sendErrorMessage: string,
) {
  const event = await signRelayEvent({
    kind: KIND_APP_ADMIN_COMMAND,
    content: serializeAppCommand(command),
    tags: [],
  });
  return relayClient.publishEventWithAck(
    event,
    timeoutMessage,
    sendErrorMessage,
  );
}

async function publishSecretCommand(
  command: AppAdminCommand,
  timeoutMessage: string,
  sendErrorMessage: string,
): Promise<AppCredentials> {
  const ack = await publishAppCommand(
    command,
    timeoutMessage,
    sendErrorMessage,
  );
  return parseAppSecretAck(ack.message, await getRelayHttpUrl());
}

export function useAppMutations() {
  const queryClient = useQueryClient();
  const relaySelf = useRelaySelfQuery().data ?? null;

  const invalidate = async () => {
    await queryClient.invalidateQueries({ queryKey: appsQueryKey(relaySelf) });
  };

  const createMutation = useMutation({
    mutationFn: (input: {
      name: string;
      description?: string;
      iconUrl?: string;
    }) =>
      publishSecretCommand(
        buildCreateCommand(input),
        "Timed out while creating the App.",
        "Failed to create the App.",
      ),
    onSuccess: invalidate,
  });

  const updateMutation = useMutation({
    mutationFn: (input: {
      appId: string;
      name?: string;
      description?: string;
      iconUrl?: string;
      clearDescription?: boolean;
      clearIcon?: boolean;
    }) =>
      publishAppCommand(
        buildUpdateCommand(input),
        "Timed out while updating the App.",
        "Failed to update the App.",
      ),
    onSuccess: invalidate,
  });

  const rotateMutation = useMutation({
    mutationFn: (appId: string) =>
      publishSecretCommand(
        buildRotateSecretCommand(appId),
        "Timed out while rotating the App secret.",
        "Failed to rotate the App secret.",
      ),
    onSuccess: invalidate,
  });

  const enableMutation = useMutation({
    mutationFn: (appId: string) =>
      publishAppCommand(
        buildEnableCommand(appId),
        "Timed out while enabling the App.",
        "Failed to enable the App.",
      ),
    onSuccess: invalidate,
  });

  const disableMutation = useMutation({
    mutationFn: (appId: string) =>
      publishAppCommand(
        buildDisableCommand(appId),
        "Timed out while disabling the App.",
        "Failed to disable the App.",
      ),
    onSuccess: invalidate,
  });

  return {
    createMutation,
    updateMutation,
    rotateMutation,
    enableMutation,
    disableMutation,
    isMutating:
      createMutation.isPending ||
      updateMutation.isPending ||
      rotateMutation.isPending ||
      enableMutation.isPending ||
      disableMutation.isPending,
  };
}
