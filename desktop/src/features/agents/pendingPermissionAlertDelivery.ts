/** Inputs for one permission-alert delivery attempt. */
export type PermissionAlertDeliveryInput = {
  agentPubkey: string;
  channelId: string | null;
  copy: { title: string; body: string };
  requestNonce: string;
  soundEnabled: boolean;
  surface: "toast" | "os" | null;
};

/** Injectable side effects for permission-alert delivery. */
export type PermissionAlertDeliveryDependencies = {
  showToast: (toast: {
    body: string;
    duration: number;
    id: string;
    onClick: () => Promise<void>;
    title: string;
  }) => void;
  sendOsNotification: (notification: {
    title: string;
    body: string;
    target: {
      agentPubkey: string;
      channelId: string | null;
      eventId: null;
      kind: null;
    };
  }) => Promise<boolean>;
  revealDesktopAppWindow: () => Promise<void>;
  openAgentActivity: (
    pubkey: string,
    options: { channelId: string | null },
  ) => unknown;
  requestDockBounce: () => Promise<void>;
  playSound: () => void;
};

/** Execute one permission-alert surface and its follow-up effects. */
export async function deliverPermissionAlert(
  input: PermissionAlertDeliveryInput,
  dependencies: PermissionAlertDeliveryDependencies,
): Promise<void> {
  if (input.surface === null) {
    return;
  }
  if (input.surface === "toast") {
    dependencies.showToast({
      body: input.copy.body,
      duration: Number.POSITIVE_INFINITY,
      id: input.requestNonce,
      title: input.copy.title,
      onClick: async () => {
        await dependencies.revealDesktopAppWindow();
        dependencies.openAgentActivity(input.agentPubkey, {
          channelId: input.channelId,
        });
      },
    });
    if (input.soundEnabled) {
      dependencies.playSound();
    }
    return;
  }

  const didSend = await dependencies.sendOsNotification({
    title: input.copy.title,
    body: input.copy.body,
    target: {
      agentPubkey: input.agentPubkey,
      channelId: input.channelId,
      eventId: null,
      kind: null,
    },
  });
  if (!didSend) {
    return;
  }
  await dependencies.requestDockBounce();
  if (input.soundEnabled) {
    dependencies.playSound();
  }
}
