import * as React from "react";
import { useLocation } from "@tanstack/react-router";
import { toast } from "sonner";

import { deriveShellRoute } from "@/app/AppShell.helpers";
import { useChannelsQuery } from "@/features/channels/hooks";
import { useChannelPanelHistoryState } from "@/features/channels/ui/useChannelPanelHistoryState";
import type { NotificationSettings } from "@/features/notifications/hooks";
import {
  requestDockBounce,
  revealDesktopAppWindow,
  sendDesktopNotification,
} from "@/features/notifications/lib/desktop";
import {
  playNotificationSound,
  resolveSlotSound,
} from "@/features/notifications/lib/sound";
import { resolveUserLabel } from "@/features/profile/lib/identity";
import { useUsersBatchQuery } from "@/features/profile/hooks";
import { isAppFocused } from "@/shared/lib/useDocumentVisible";

import {
  getAgentTranscript,
  isAgentObserverInitialReplayComplete,
  subscribeAgentObserverStore,
  type AgentObserverStoreUpdate,
} from "./observerRelayStore";
import {
  applyPermissionAlertStoreNotification,
  createPermissionAlertStoreState,
  permissionAlertCopy,
  selectPermissionAlertSurface,
  shouldSuppressPermissionAlert,
  startPermissionAlertStoreSubscription,
} from "./pendingPermissionAlert";
import { deliverPermissionAlert } from "./pendingPermissionAlertDelivery";
import { useObserverIngestionAgents } from "./useAgentObserverIngestion";
import { useOpenAgentActivity } from "./useOpenAgentActivity";

/** Deliver app-level owner permission alerts from the observer store. */
export function usePendingPermissionAlerts({
  enabled,
  notificationSettings,
}: {
  enabled: boolean;
  notificationSettings: NotificationSettings;
}): void {
  const ingestionAgents = useObserverIngestionAgents();
  const agentPubkeys = React.useMemo(
    () => ingestionAgents.map((agent) => agent.pubkey),
    [ingestionAgents],
  );
  const profilesQuery = useUsersBatchQuery(agentPubkeys, {
    enabled: agentPubkeys.length > 0,
  });
  const channelsQuery = useChannelsQuery();
  const channels = channelsQuery.data ?? [];
  const { openAgentActivity } = useOpenAgentActivity();
  const location = useLocation();
  const { selectedChannelId } = deriveShellRoute(location.pathname);
  const { openAgentSessionChannelId, openAgentSessionPubkey } =
    useChannelPanelHistoryState();
  const [alertState] = React.useState(createPermissionAlertStoreState);

  const handleStoreUpdate = React.useEffectEvent(
    (update?: AgentObserverStoreUpdate) => {
      const applied = applyPermissionAlertStoreNotification({
        state: alertState,
        initialReplayComplete: isAgentObserverInitialReplayComplete(),
        agents: ingestionAgents.map((agent) => ({
          pubkey: agent.pubkey,
          transcript: getAgentTranscript(agent.pubkey),
        })),
        update,
      });

      alertState.seededAgentPubkeys.clear();
      for (const pubkey of applied.nextState.seededAgentPubkeys) {
        alertState.seededAgentPubkeys.add(pubkey);
      }
      alertState.seenNonces.clear();
      for (const nonce of applied.nextState.seenNonces) {
        alertState.seenNonces.add(nonce);
      }

      for (const nonce of applied.dismissNonces) {
        toast.dismiss(nonce);
      }

      if (!update) {
        return;
      }

      const agentName = resolveUserLabel({
        pubkey: update.agentPubkey,
        profiles: profilesQuery.data?.profiles,
        preferResolvedSelfLabel: true,
      });

      for (const alert of applied.alerts) {
        if (
          shouldSuppressPermissionAlert({
            agentPubkey: update.agentPubkey,
            channelId: alert.channelId,
            openAgentSession: openAgentSessionPubkey,
            openAgentSessionChannel: openAgentSessionChannelId,
            currentChannelId: selectedChannelId,
          })
        ) {
          continue;
        }

        const channel = channels.find((entry) => entry.id === alert.channelId);
        const copy = permissionAlertCopy({
          agentName,
          channelName: channel?.name ?? null,
        });
        const surface = selectPermissionAlertSurface({
          focused: isAppFocused(),
          desktopEnabled: notificationSettings.desktopEnabled,
        });

        void deliverPermissionAlert(
          {
            agentPubkey: update.agentPubkey,
            channelId: alert.channelId,
            copy,
            requestNonce: alert.requestNonce,
            soundEnabled: notificationSettings.slotAlertsEnabled.needs_action,
            surface,
          },
          {
            showToast: (alertToast) => {
              // Sonner has no toast-body onClick; map the delivery click
              // through the action button (same pattern as other alerts).
              toast(alertToast.title, {
                id: alertToast.id,
                description: alertToast.body,
                duration: alertToast.duration,
                action: {
                  label: "Open",
                  onClick: (event) => {
                    event.preventDefault();
                    void alertToast.onClick();
                  },
                },
              });
            },
            sendOsNotification: sendDesktopNotification,
            revealDesktopAppWindow,
            openAgentActivity,
            requestDockBounce,
            playSound: () => {
              playNotificationSound(
                resolveSlotSound(notificationSettings, "needs_action"),
              );
            },
          },
        );
      }
    },
  );

  // biome-ignore lint/correctness/useExhaustiveDependencies: re-subscribe and re-seed when the owned-agent ingestion set changes
  React.useEffect(() => {
    if (!enabled) {
      return;
    }
    return startPermissionAlertStoreSubscription({
      handleUpdate: handleStoreUpdate,
      subscribe: subscribeAgentObserverStore,
    });
  }, [enabled, ingestionAgents]);
}
