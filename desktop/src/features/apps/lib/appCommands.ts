import { parseCanonicalAppId } from "@/features/apps/lib/appMetadata";
import { KIND_APP_ADMIN_COMMAND as KIND } from "@/shared/constants/kinds";

export const KIND_APP_ADMIN_COMMAND = KIND;

const APP_NAME_MAX_CHARS = 128;
const APP_DESCRIPTION_MAX_CHARS = 2_048;
const APP_ICON_URL_MAX_BYTES = 4_096;
const DUPLICATE_NO_SECRET_MSG =
  "duplicate create or rotate returned no secret; use rotate-secret";

export type AppCreateCommand = {
  action: "create";
  name: string;
  description?: string;
  icon_url?: string;
};

export type AppUpdateCommand = {
  action: "update";
  app_id: string;
  name?: string;
  description?: string;
  icon_url?: string;
};

export type AppRotateSecretCommand = {
  action: "rotate_secret";
  app_id: string;
};

export type AppEnableCommand = {
  action: "enable";
  app_id: string;
};

export type AppDisableCommand = {
  action: "disable";
  app_id: string;
};

export type AppAdminCommand =
  | AppCreateCommand
  | AppUpdateCommand
  | AppRotateSecretCommand
  | AppEnableCommand
  | AppDisableCommand;

export type AppCredentials = {
  appId: string;
  callbackUrl: string;
  webhookSecret: string;
};

/** Mutation/query cache payload for create/rotate. Never includes the secret. */
export type AppPublicAck = {
  appId: string;
  callbackUrl: string;
};

export type AppCredentialStore = {
  get: () => AppCredentials | null;
  set: (credentials: AppCredentials) => void;
  clearOnClose: () => void;
  clearOnUnmount: () => void;
};

function requireCanonicalAppId(appId: string): string {
  const parsed = parseCanonicalAppId(appId);
  if (!parsed) {
    throw new Error("app id must be a canonical lowercase UUID");
  }
  return parsed;
}

function validateName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) {
    throw new Error("name must be non-empty");
  }
  if ([...trimmed].length > APP_NAME_MAX_CHARS) {
    throw new Error(`name exceeds ${APP_NAME_MAX_CHARS} Unicode scalar values`);
  }
  return trimmed;
}

function validateDescription(description: string): string {
  if ([...description].length > APP_DESCRIPTION_MAX_CHARS) {
    throw new Error(
      `description exceeds ${APP_DESCRIPTION_MAX_CHARS} Unicode scalar values`,
    );
  }
  return description;
}

function validateIconUrl(iconUrl: string): string {
  if (iconUrl.length === 0) {
    return iconUrl;
  }
  if (iconUrl.length > APP_ICON_URL_MAX_BYTES) {
    throw new Error(`icon_url exceeds ${APP_ICON_URL_MAX_BYTES} UTF-8 bytes`);
  }
  for (const char of iconUrl) {
    const code = char.charCodeAt(0);
    if (code <= 32) {
      throw new Error("icon_url contains invalid characters");
    }
  }
  if (
    iconUrl.startsWith("data:image/") ||
    iconUrl.startsWith("https://") ||
    iconUrl.startsWith("http://")
  ) {
    return iconUrl;
  }
  throw new Error("icon_url must be an http(s) URL or data:image/ URL");
}

export function appCallbackUrl(relayHttpUrl: string, appId: string): string {
  return `${relayHttpUrl.replace(/\/+$/, "")}/hooks/apps/${appId}`;
}

export function buildCreateCommand(input: {
  name: string;
  description?: string;
  iconUrl?: string;
}): AppCreateCommand {
  const command: AppCreateCommand = {
    action: "create",
    name: validateName(input.name),
  };
  if (input.description) {
    command.description = validateDescription(input.description);
  }
  if (input.iconUrl) {
    command.icon_url = validateIconUrl(input.iconUrl);
  }
  return command;
}

export function buildUpdateCommand(input: {
  appId: string;
  name?: string;
  description?: string;
  iconUrl?: string;
  clearDescription?: boolean;
  clearIcon?: boolean;
}): AppUpdateCommand {
  const command: AppUpdateCommand = {
    action: "update",
    app_id: requireCanonicalAppId(input.appId),
  };
  if (input.name !== undefined) {
    command.name = validateName(input.name);
  }
  if (input.clearDescription) {
    command.description = "";
  } else if (input.description !== undefined) {
    command.description = validateDescription(input.description);
  }
  if (input.clearIcon) {
    command.icon_url = "";
  } else if (input.iconUrl !== undefined) {
    command.icon_url = validateIconUrl(input.iconUrl);
  }
  if (
    command.name === undefined &&
    command.description === undefined &&
    command.icon_url === undefined
  ) {
    throw new Error(
      "update must contain at least one of name, description, or icon_url",
    );
  }
  return command;
}

export function buildRotateSecretCommand(
  appId: string,
): AppRotateSecretCommand {
  return {
    action: "rotate_secret",
    app_id: requireCanonicalAppId(appId),
  };
}

export function buildEnableCommand(appId: string): AppEnableCommand {
  return { action: "enable", app_id: requireCanonicalAppId(appId) };
}

export function buildDisableCommand(appId: string): AppDisableCommand {
  return { action: "disable", app_id: requireCanonicalAppId(appId) };
}

export function serializeAppCommand(command: AppAdminCommand): string {
  const body: Record<string, unknown> = { ...command };
  for (const [key, value] of Object.entries(body)) {
    if (value === undefined || value === null) {
      delete body[key];
    }
  }
  return JSON.stringify(body);
}

export function parseAppSecretAck(
  message: string,
  relayHttpUrl: string,
): AppCredentials {
  if (message === "duplicate" || message.startsWith("duplicate:")) {
    throw new Error(DUPLICATE_NO_SECRET_MSG);
  }
  const payloadJson = message.startsWith("response:")
    ? message.slice("response:".length)
    : null;
  let payload: { app_id?: unknown; webhook_secret?: unknown };
  try {
    payload = payloadJson ? (JSON.parse(payloadJson) as typeof payload) : {};
  } catch {
    throw new Error(`relay response missing response: payload: ${message}`);
  }
  if (!payloadJson) {
    throw new Error(`relay response missing response: payload: ${message}`);
  }
  const appId =
    typeof payload.app_id === "string"
      ? parseCanonicalAppId(payload.app_id)
      : null;
  if (!appId) {
    throw new Error("relay response missing app_id");
  }
  const secret =
    typeof payload.webhook_secret === "string" ? payload.webhook_secret : "";
  if (!secret) {
    throw new Error(
      `create or rotate returned no secret; use rotate-secret --app ${appId}`,
    );
  }
  return {
    appId,
    callbackUrl: appCallbackUrl(relayHttpUrl, appId),
    webhookSecret: secret,
  };
}

export function createAppCredentialStore(): AppCredentialStore {
  let credentials: AppCredentials | null = null;
  const clear = () => {
    credentials = null;
  };
  return {
    get: () => credentials,
    set: (next) => {
      credentials = next;
    },
    clearOnClose: clear,
    clearOnUnmount: clear,
  };
}

export function publicAppAck(credentials: AppCredentials): AppPublicAck {
  return {
    appId: credentials.appId,
    callbackUrl: credentials.callbackUrl,
  };
}

export function mutationHoldsAppSecret(data: unknown): boolean {
  if (!data || typeof data !== "object") {
    return false;
  }
  const secret = (data as { webhookSecret?: unknown }).webhookSecret;
  return typeof secret === "string" && secret.length > 0;
}

export function closeAppCredentialCaches({
  store,
  mutationData,
}: {
  store?: AppCredentialStore;
  mutationData?: unknown;
}): { mutationData: null } {
  store?.clearOnClose();
  if (mutationHoldsAppSecret(mutationData)) {
    return { mutationData: null };
  }
  return { mutationData: null };
}
