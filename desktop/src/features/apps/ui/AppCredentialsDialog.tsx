import * as React from "react";

import {
  type AppCredentials,
  createAppCredentialStore,
} from "@/features/apps/lib/appCommands";
import { copyTextToClipboard } from "@/shared/lib/clipboard";
import { Button } from "@/shared/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/shared/ui/dialog";

type AppCredentialsDialogProps = {
  credentials: AppCredentials | null;
  onOpenChange: (open: boolean) => void;
};

export function AppCredentialsDialog({
  credentials,
  onOpenChange,
}: AppCredentialsDialogProps) {
  const storeRef = React.useRef(createAppCredentialStore());
  const [secret, setSecret] = React.useState("");
  const [callbackUrl, setCallbackUrl] = React.useState("");
  const open = credentials !== null;

  React.useEffect(() => {
    if (!credentials) {
      return;
    }
    storeRef.current.set(credentials);
    setSecret(credentials.webhookSecret);
    setCallbackUrl(credentials.callbackUrl);
  }, [credentials]);

  React.useEffect(() => {
    return () => {
      storeRef.current.clearOnUnmount();
      setSecret("");
      setCallbackUrl("");
    };
  }, []);

  function handleOpenChange(next: boolean) {
    if (!next) {
      storeRef.current.clearOnClose();
      setSecret("");
      setCallbackUrl("");
    }
    onOpenChange(next);
  }

  return (
    <Dialog onOpenChange={handleOpenChange} open={open}>
      <DialogContent
        className="sm:max-w-lg"
        data-testid="apps-credentials-dialog"
      >
        <DialogHeader>
          <DialogTitle>App credentials</DialogTitle>
          <DialogDescription>
            This secret is shown only once. Copy it now. Closing this dialog
            removes it from the app.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4">
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground">
              Callback URL
            </p>
            <pre
              className="overflow-x-auto rounded-md bg-muted/50 p-3 font-mono text-xs"
              data-testid="apps-credentials-callback"
            >
              {callbackUrl}
            </pre>
            <Button
              data-testid="apps-copy-callback"
              onClick={() => copyTextToClipboard(callbackUrl)}
              size="sm"
              type="button"
              variant="outline"
            >
              Copy URL
            </Button>
          </div>
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground">
              X-Webhook-Secret
            </p>
            <pre
              className="overflow-x-auto rounded-md bg-muted/50 p-3 font-mono text-xs"
              data-testid="apps-credentials-secret"
            >
              {secret}
            </pre>
            <Button
              data-testid="apps-copy-secret"
              onClick={() => copyTextToClipboard(secret)}
              size="sm"
              type="button"
              variant="outline"
            >
              Copy secret
            </Button>
          </div>
        </div>
        <DialogFooter>
          <Button
            data-testid="apps-credentials-close"
            onClick={() => handleOpenChange(false)}
            type="button"
          >
            Done
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
