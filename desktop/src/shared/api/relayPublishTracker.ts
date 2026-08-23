import type { RelayEvent } from "@/shared/api/types";
import { PUBLISH_TIMEOUT_MS } from "@/shared/api/relayClientTimings";

export type RelayPublishAck = {
  event: RelayEvent;
  message: string;
};

export type RelayPublishTimerApi = {
  setTimeout: (fn: () => void, ms: number) => number;
  clearTimeout: (id: number) => void;
};

export type PendingPublish = {
  event: RelayEvent;
  resolve: (ack: RelayPublishAck) => void;
  reject: (error: Error) => void;
  timeout: number;
};

export type PublishWithAckOptions = {
  tracker: RelayPublishTracker;
  event: RelayEvent;
  timeoutMs?: number;
  timeoutMessage: string;
  sendErrorMessage: string;
  waitForGate: () => Promise<void>;
  sendEvent: (event: RelayEvent) => Promise<void>;
  ensureConnected: () => Promise<void>;
  recoverFromFailure: (error: unknown, fallback: string) => Error;
};

function defaultTimers(): RelayPublishTimerApi {
  return {
    setTimeout: (fn, ms) => window.setTimeout(fn, ms),
    clearTimeout: (id) => {
      window.clearTimeout(id);
    },
  };
}

export class RelayPublishTracker {
  private pending = new Map<string, PendingPublish>();
  private readonly timers: RelayPublishTimerApi;

  constructor(timers: RelayPublishTimerApi = defaultTimers()) {
    this.timers = timers;
  }

  begin(
    event: RelayEvent,
    options: { timeoutMs: number; timeoutMessage: string },
  ): Promise<RelayPublishAck> {
    return new Promise<RelayPublishAck>((resolve, reject) => {
      const timeout = this.timers.setTimeout(() => {
        this.pending.delete(event.id);
        reject(new Error(options.timeoutMessage));
      }, options.timeoutMs);
      this.pending.set(event.id, {
        event,
        resolve,
        reject,
        timeout,
      });
    });
  }

  get(eventId: string): PendingPublish | undefined {
    return this.pending.get(eventId);
  }

  take(eventId: string): PendingPublish | undefined {
    const pending = this.pending.get(eventId);
    if (!pending) {
      return undefined;
    }
    this.pending.delete(eventId);
    return pending;
  }

  restore(eventId: string, pending: PendingPublish): void {
    this.pending.set(eventId, pending);
  }

  handleOk(eventId: string, success: boolean, message: string): boolean {
    const pending = this.pending.get(eventId);
    if (!pending) {
      return false;
    }
    this.timers.clearTimeout(pending.timeout);
    this.pending.delete(eventId);
    if (success) {
      pending.resolve({ event: pending.event, message });
    } else {
      pending.reject(new Error(message || "Relay rejected the event."));
    }
    return true;
  }

  clearTimeout(pending: PendingPublish): void {
    this.timers.clearTimeout(pending.timeout);
  }

  rejectAll(error: Error): void {
    for (const [eventId, pending] of this.pending) {
      this.timers.clearTimeout(pending.timeout);
      pending.reject(error);
      this.pending.delete(eventId);
    }
  }
}

export async function publishWithAck({
  tracker,
  event,
  timeoutMs = PUBLISH_TIMEOUT_MS,
  timeoutMessage,
  sendErrorMessage,
  waitForGate,
  sendEvent,
  ensureConnected,
  recoverFromFailure,
}: PublishWithAckOptions): Promise<RelayPublishAck> {
  await waitForGate();
  const ack = tracker.begin(event, { timeoutMs, timeoutMessage });
  void sendEvent(event).catch(async (error) => {
    const pendingEvent = tracker.take(event.id);
    const normalizedError = recoverFromFailure(error, sendErrorMessage);
    try {
      await ensureConnected();
      if (!pendingEvent) {
        throw normalizedError;
      }
      tracker.restore(event.id, pendingEvent);
      await sendEvent(event);
    } catch (retryError) {
      if (pendingEvent) {
        tracker.clearTimeout(pendingEvent);
      }
      tracker.take(event.id);
      const rejected = recoverFromFailure(retryError, normalizedError.message);
      pendingEvent?.reject(rejected);
    }
  });
  return ack;
}
