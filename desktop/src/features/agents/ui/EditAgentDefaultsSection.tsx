import * as React from "react";

import { AgentAiDefaultsNotice } from "./AgentAiDefaults";
import { AgentDefaultsDialog } from "./AgentDefaultsDialog";
import type { InheritedDefault } from "./bakedEnvHelpers";

export function EditAgentDefaultsSection({
  explicitModel,
  explicitProvider,
  inheritedModel,
  inheritedProvider,
}: {
  explicitModel: string;
  explicitProvider: string;
  inheritedModel: InheritedDefault;
  inheritedProvider: InheritedDefault;
}) {
  const [open, setOpen] = React.useState(false);
  const triggerRef = React.useRef<HTMLButtonElement>(null);

  return (
    <>
      <AgentAiDefaultsNotice
        onEditDefaults={() => setOpen(true)}
        triggerRef={triggerRef}
        explicitModel={explicitModel}
        explicitProvider={explicitProvider}
        inheritedModel={inheritedModel}
        inheritedProvider={inheritedProvider}
      />
      <AgentDefaultsDialog
        onOpenChange={setOpen}
        open={open}
        returnFocusRef={triggerRef}
      />
    </>
  );
}
