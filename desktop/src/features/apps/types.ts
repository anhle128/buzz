export type AppMetadata = {
  appId: string;
  name: string;
  description?: string;
  picture?: string;
  status: "active" | "disabled";
  eventId: string;
  relayPubkey: string;
  updatedAt: number;
};

export type AppActor = {
  appId: string;
  name: string;
  picture?: string;
  signerPubkey: string;
};

export type ResolvedMessageActor =
  | ({ type: "app" } & AppActor)
  | { type: "user"; pubkey: string };

export type AppActorMode = "live" | "search" | "feed";
