import * as React from "react";

import type { AppMetadata } from "@/features/apps/types";
import { uploadMediaBytes } from "@/shared/api/tauri";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";
import { Input } from "@/shared/ui/input";
import { Textarea } from "@/shared/ui/textarea";

const IMAGE_TYPES = ["image/gif", "image/jpeg", "image/png", "image/webp"];

export type AppFormValues = {
  name: string;
  description?: string;
  iconUrl?: string;
  clearDescription?: boolean;
  clearIcon?: boolean;
};

type AppFormDialogProps = {
  app: AppMetadata | null;
  errorMessage: string | null;
  mode: "create" | "edit";
  onOpenChange: (open: boolean) => void;
  onSubmit: (values: AppFormValues) => Promise<void>;
  open: boolean;
  pending: boolean;
};

export function AppFormDialog({
  app,
  errorMessage,
  mode,
  onOpenChange,
  onSubmit,
  open,
  pending,
}: AppFormDialogProps) {
  const fileInputRef = React.useRef<HTMLInputElement | null>(null);
  const [name, setName] = React.useState("");
  const [description, setDescription] = React.useState("");
  const [iconUrl, setIconUrl] = React.useState<string | undefined>();
  const [clearDescription, setClearDescription] = React.useState(false);
  const [clearIcon, setClearIcon] = React.useState(false);
  const [localError, setLocalError] = React.useState<string | null>(null);
  const [uploading, setUploading] = React.useState(false);
  const isCreate = mode === "create";
  const busy = pending || uploading;
  const visibleError = localError ?? errorMessage;

  React.useEffect(() => {
    if (!open) {
      return;
    }
    setName(app?.name ?? "");
    setDescription(app?.description ?? "");
    setIconUrl(app?.picture);
    setClearDescription(false);
    setClearIcon(false);
    setLocalError(null);
  }, [app, open]);

  async function handleIconFile(file: File | undefined) {
    if (!file) {
      return;
    }
    if (!IMAGE_TYPES.includes(file.type)) {
      setLocalError("Choose a PNG, JPG, GIF, or WebP image.");
      return;
    }
    setUploading(true);
    setLocalError(null);
    try {
      const buffer = await file.arrayBuffer();
      const uploaded = await uploadMediaBytes(
        [...new Uint8Array(buffer)],
        file.name,
      );
      if (!uploaded.type.startsWith("image/")) {
        setLocalError("Choose a PNG, JPG, GIF, or WebP image.");
        return;
      }
      setIconUrl(uploaded.url);
      setClearIcon(false);
    } catch (error) {
      setLocalError(
        error instanceof Error ? error.message : "Could not upload that icon.",
      );
    } finally {
      setUploading(false);
    }
  }

  async function handleSubmit(event: React.FormEvent) {
    event.preventDefault();
    setLocalError(null);
    try {
      await onSubmit({
        name,
        description: clearDescription ? undefined : description,
        iconUrl: clearIcon ? undefined : iconUrl,
        clearDescription: !isCreate && clearDescription,
        clearIcon: !isCreate && clearIcon,
      });
    } catch (error) {
      setLocalError(
        error instanceof Error ? error.message : "Could not save the App.",
      );
    }
  }

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="sm:max-w-lg" data-testid="apps-form-dialog">
        <form
          className="space-y-4"
          onSubmit={(event) => void handleSubmit(event)}
        >
          <DialogHeader>
            <DialogTitle>{isCreate ? "Create App" : "Edit App"}</DialogTitle>
            <DialogDescription>
              {isCreate
                ? "Apps receive external callbacks and post notifications into project channels."
                : "Update the public App name, description, or icon."}
            </DialogDescription>
          </DialogHeader>

          <div className="space-y-2">
            <label className="text-sm font-medium" htmlFor="apps-form-name">
              Name
            </label>
            <Input
              data-testid="apps-form-name"
              disabled={busy}
              id="apps-form-name"
              onChange={(event) => setName(event.target.value)}
              value={name}
            />
          </div>

          <div className="space-y-2">
            <label
              className="text-sm font-medium"
              htmlFor="apps-form-description"
            >
              Description
            </label>
            <Textarea
              data-testid="apps-form-description"
              disabled={busy || clearDescription}
              id="apps-form-description"
              onChange={(event) => setDescription(event.target.value)}
              value={clearDescription ? "" : description}
            />
            {!isCreate ? (
              <Button
                data-testid="apps-form-clear-description"
                disabled={busy}
                onClick={() => {
                  setClearDescription(true);
                  setDescription("");
                }}
                size="sm"
                type="button"
                variant="ghost"
              >
                Clear description
              </Button>
            ) : null}
          </div>

          <div className="space-y-2">
            <p className="text-sm font-medium">Icon</p>
            <input
              accept="image/png,image/jpeg,image/gif,image/webp"
              className="sr-only"
              data-testid="apps-form-icon-input"
              disabled={busy}
              onChange={(event) => {
                const file = event.target.files?.[0];
                event.target.value = "";
                void handleIconFile(file);
              }}
              ref={fileInputRef}
              type="file"
            />
            <div className="flex items-center gap-2">
              <Button
                disabled={busy}
                onClick={() => fileInputRef.current?.click()}
                size="sm"
                type="button"
                variant="outline"
              >
                Upload icon
              </Button>
              {!isCreate ? (
                <Button
                  data-testid="apps-form-clear-icon"
                  disabled={busy}
                  onClick={() => {
                    setClearIcon(true);
                    setIconUrl(undefined);
                  }}
                  size="sm"
                  type="button"
                  variant="ghost"
                >
                  Clear icon
                </Button>
              ) : null}
            </div>
            {iconUrl && !clearIcon ? (
              <img
                alt=""
                className="h-10 w-10 rounded-md object-cover"
                data-testid="apps-form-icon"
                src={iconUrl}
              />
            ) : null}
          </div>

          {visibleError ? (
            <p
              className="text-sm text-destructive"
              data-testid="apps-form-validation-error"
            >
              {visibleError}
            </p>
          ) : null}
          {pending ? (
            <p
              className="text-sm text-muted-foreground"
              data-testid="apps-form-pending"
            >
              Waiting for the relay to accept this App command…
            </p>
          ) : null}

          <DialogFooter>
            <Button
              data-testid="apps-form-submit"
              disabled={busy}
              type="submit"
            >
              {pending
                ? isCreate
                  ? "Creating…"
                  : "Saving…"
                : isCreate
                  ? "Create"
                  : "Save"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
