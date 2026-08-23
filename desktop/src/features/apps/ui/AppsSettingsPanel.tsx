import { useQuery } from "@tanstack/react-query";
import * as React from "react";

import { useAppMutations } from "@/features/apps/hooks/useAppMutations";
import { useAppsQuery } from "@/features/apps/hooks/useAppsQuery";
import {
  appCallbackUrl,
  type AppCredentials,
} from "@/features/apps/lib/appCommands";
import type { AppMetadata } from "@/features/apps/types";
import { AppCredentialsDialog } from "@/features/apps/ui/AppCredentialsDialog";
import {
  AppFormDialog,
  type AppFormValues,
} from "@/features/apps/ui/AppFormDialog";
import { useMyRelayMembershipLookupQuery } from "@/features/community-members/hooks";
import { SettingsOptionGroup } from "@/features/settings/ui/SettingsOptionGroup";
import { SettingsSectionHeader } from "@/features/settings/ui/SettingsSectionHeader";
import { canManageCommunityMembers } from "@/shared/api/relayMembers";
import { getRelayHttpUrl } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/shared/ui/alert-dialog";
import { Spinner } from "@/shared/ui/spinner";

function formatUpdatedAt(updatedAt: number): string {
  return new Date(updatedAt * 1_000).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

function AppRow({
  app,
  callbackUrl,
  disablePending,
  enablePending,
  mutating,
  onDisable,
  onEdit,
  onEnable,
  onRotate,
}: {
  app: AppMetadata;
  callbackUrl: string;
  disablePending: boolean;
  enablePending: boolean;
  mutating: boolean;
  onDisable: () => void;
  onEdit: () => void;
  onEnable: () => void;
  onRotate: () => void;
}) {
  const isActive = app.status === "active";
  return (
    <div
      className="flex flex-col gap-3 px-4 py-4 sm:flex-row sm:items-start sm:justify-between"
      data-testid={`apps-row-${app.appId}`}
    >
      <div className="flex min-w-0 items-start gap-3">
        {app.picture ? (
          <img
            alt=""
            className="h-10 w-10 shrink-0 rounded-md object-cover"
            data-testid="apps-row-icon"
            src={app.picture}
          />
        ) : null}
        <div className="min-w-0 space-y-1">
          <p className="text-sm font-medium" data-testid="apps-row-name">
            {app.name}
          </p>
          {app.description ? (
            <p
              className="text-sm text-muted-foreground"
              data-testid="apps-row-description"
            >
              {app.description}
            </p>
          ) : null}
          <p className="text-sm text-muted-foreground">
            Status{" "}
            <span data-testid="apps-row-status">
              {isActive ? "Active" : "Disabled"}
            </span>
          </p>
          <p className="text-sm text-muted-foreground">
            Updated{" "}
            <span data-testid="apps-row-updated">
              {formatUpdatedAt(app.updatedAt)}
            </span>
          </p>
          <p
            className="break-all font-mono text-2xs text-muted-foreground"
            data-testid="apps-row-callback"
          >
            {callbackUrl}
          </p>
        </div>
      </div>
      <div className="flex shrink-0 flex-wrap gap-2">
        <Button
          data-testid="apps-edit-button"
          disabled={mutating}
          onClick={onEdit}
          size="sm"
          type="button"
          variant="outline"
        >
          Edit
        </Button>
        <Button
          data-testid="apps-rotate-button"
          disabled={mutating}
          onClick={onRotate}
          size="sm"
          type="button"
          variant="outline"
        >
          Rotate secret
        </Button>
        {isActive ? (
          <Button
            data-testid="apps-disable-button"
            disabled={mutating || disablePending}
            onClick={onDisable}
            size="sm"
            type="button"
            variant="outline"
          >
            Disable
          </Button>
        ) : (
          <Button
            data-testid="apps-enable-button"
            disabled={mutating || enablePending}
            onClick={onEnable}
            size="sm"
            type="button"
          >
            Enable
          </Button>
        )}
      </div>
    </div>
  );
}

export function AppsSettingsPanel() {
  const membershipQuery = useMyRelayMembershipLookupQuery();
  const appsQuery = useAppsQuery();
  const relayHttpQuery = useQuery({
    queryKey: ["relayHttpUrl"],
    queryFn: getRelayHttpUrl,
    staleTime: Number.POSITIVE_INFINITY,
  });
  const mutations = useAppMutations();
  const [formMode, setFormMode] = React.useState<"create" | "edit" | null>(
    null,
  );
  const [editingApp, setEditingApp] = React.useState<AppMetadata | null>(null);
  const [credentials, setCredentials] = React.useState<AppCredentials | null>(
    null,
  );
  const [rotateApp, setRotateApp] = React.useState<AppMetadata | null>(null);
  const [disableApp, setDisableApp] = React.useState<AppMetadata | null>(null);

  const canManage = canManageCommunityMembers(membershipQuery.data);
  const apps = React.useMemo(
    () =>
      [...appsQuery.data.values()].sort((left, right) =>
        left.name.localeCompare(right.name),
      ),
    [appsQuery.data],
  );
  const relayHttpUrl = relayHttpQuery.data ?? "";

  if (membershipQuery.isPending) {
    return (
      <section className="min-w-0" data-testid="settings-apps">
        <p className="text-sm text-muted-foreground">
          Checking App management permissions…
        </p>
      </section>
    );
  }

  if (membershipQuery.isError || !canManage) {
    return (
      <section className="min-w-0" data-testid="apps-settings-unavailable">
        <p className="text-sm text-muted-foreground">
          Apps settings are unavailable.
        </p>
      </section>
    );
  }

  async function handleCreate(values: AppFormValues) {
    const created = await mutations.createMutation.mutateAsync({
      name: values.name,
      description: values.description,
      iconUrl: values.iconUrl,
    });
    setFormMode(null);
    setCredentials(created);
  }

  async function handleEdit(values: AppFormValues) {
    if (!editingApp) {
      return;
    }
    await mutations.updateMutation.mutateAsync({
      appId: editingApp.appId,
      name: values.name,
      description: values.description,
      iconUrl: values.iconUrl,
      clearDescription: values.clearDescription,
      clearIcon: values.clearIcon,
    });
    setFormMode(null);
    setEditingApp(null);
  }

  const listUnavailable = appsQuery.isLoading || appsQuery.isError;
  const formPending =
    mutations.createMutation.isPending || mutations.updateMutation.isPending;

  return (
    <section className="min-w-0" data-testid="settings-apps">
      <SettingsSectionHeader
        action={
          <Button
            data-testid="apps-create-button"
            disabled={listUnavailable || mutations.isMutating}
            onClick={() => {
              setEditingApp(null);
              setFormMode("create");
            }}
            type="button"
          >
            Create App
          </Button>
        }
        description="Apps receive external callbacks and post notifications into project channels."
        title="Apps"
      />

      {appsQuery.isLoading ? (
        <div
          className="flex items-center gap-2 text-sm text-muted-foreground"
          data-testid="apps-settings-loading"
        >
          <Spinner className="h-4 w-4" />
          Loading Apps…
        </div>
      ) : null}

      {appsQuery.isError ? (
        <p
          className="rounded-md border border-destructive/30 bg-destructive/10 px-3 py-2 text-sm text-destructive"
          data-testid="apps-settings-error"
        >
          {appsQuery.error instanceof Error
            ? appsQuery.error.message
            : "Could not load Apps."}
        </p>
      ) : null}

      {!appsQuery.isLoading && !appsQuery.isError && apps.length === 0 ? (
        <p
          className="rounded-lg border border-dashed border-border/70 px-3 py-6 text-center text-sm text-muted-foreground"
          data-testid="apps-settings-empty"
        >
          No Apps yet. Create an App to receive external callbacks and post
          notifications into project channels.
        </p>
      ) : null}

      {apps.length > 0 ? (
        <SettingsOptionGroup>
          {apps.map((app) => (
            <AppRow
              app={app}
              callbackUrl={
                relayHttpUrl ? appCallbackUrl(relayHttpUrl, app.appId) : ""
              }
              disablePending={mutations.disableMutation.isPending}
              enablePending={mutations.enableMutation.isPending}
              key={app.appId}
              mutating={mutations.isMutating}
              onDisable={() => setDisableApp(app)}
              onEdit={() => {
                setEditingApp(app);
                setFormMode("edit");
              }}
              onEnable={() =>
                void mutations.enableMutation.mutateAsync(app.appId)
              }
              onRotate={() => setRotateApp(app)}
            />
          ))}
        </SettingsOptionGroup>
      ) : null}

      <AppFormDialog
        app={formMode === "edit" ? editingApp : null}
        errorMessage={
          formMode === "create"
            ? mutations.createMutation.error instanceof Error
              ? mutations.createMutation.error.message
              : null
            : mutations.updateMutation.error instanceof Error
              ? mutations.updateMutation.error.message
              : null
        }
        mode={formMode === "edit" ? "edit" : "create"}
        onOpenChange={(open) => {
          if (!open) {
            setFormMode(null);
            setEditingApp(null);
          }
        }}
        onSubmit={formMode === "edit" ? handleEdit : handleCreate}
        open={formMode !== null}
        pending={formPending}
      />

      <AppCredentialsDialog
        credentials={credentials}
        onOpenChange={(open) => {
          if (!open) setCredentials(null);
        }}
      />

      <AlertDialog
        onOpenChange={(open) => {
          if (!open) setRotateApp(null);
        }}
        open={rotateApp !== null}
      >
        <AlertDialogContent data-testid="apps-rotate-confirm">
          <AlertDialogHeader>
            <AlertDialogTitle>Rotate this App secret?</AlertDialogTitle>
            <AlertDialogDescription>
              The current callback secret stops working immediately. The new
              secret is shown only once.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              data-testid="apps-rotate-confirm-accept"
              onClick={() => {
                if (!rotateApp) return;
                void mutations.rotateMutation
                  .mutateAsync(rotateApp.appId)
                  .then((next) => {
                    setCredentials(next);
                    setRotateApp(null);
                  });
              }}
            >
              Rotate secret
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>

      <AlertDialog
        onOpenChange={(open) => {
          if (!open) setDisableApp(null);
        }}
        open={disableApp !== null}
      >
        <AlertDialogContent data-testid="apps-disable-confirm">
          <AlertDialogHeader>
            <AlertDialogTitle>Disable this App?</AlertDialogTitle>
            <AlertDialogDescription>
              Callbacks will be rejected. Public metadata stays available so
              existing messages keep their App attribution.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              data-testid="apps-disable-confirm-accept"
              onClick={() => {
                if (!disableApp) return;
                void mutations.disableMutation
                  .mutateAsync(disableApp.appId)
                  .then(() => setDisableApp(null));
              }}
            >
              Disable
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </section>
  );
}
