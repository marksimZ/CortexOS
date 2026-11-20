import type { Message } from "./studio";

export interface StreamUpdate {
  workspaceId: string;
  conversationId: string;
  runId: string;
  messageId: string;
  seq: number;
  content: string;
  status: string;
  errorMessage: string | null;
}

const paintIntervalMs = 90;

export function createChatStream() {
  const delivered = new Map<string, StreamUpdate>();
  const subscribers = new Map<string, Set<() => void>>();
  let pending: StreamUpdate | undefined;
  let timer: number | undefined;

  function publish(update: StreamUpdate) {
    delivered.set(update.messageId, update);
    subscribers.get(update.messageId)?.forEach(notify => notify());
  }

  function cancelPending() {
    window.clearTimeout(timer);
    timer = undefined;
    pending = undefined;
  }

  function forget(messageId: string) {
    if (pending?.messageId === messageId) cancelPending();
    if (delivered.delete(messageId)) subscribers.get(messageId)?.forEach(notify => notify());
  }

  return {
    forget,
    snapshot: (messageId: string) => delivered.get(messageId),
    subscribe(messageId: string, notify: () => void) {
      const listeners = subscribers.get(messageId) ?? new Set();
      subscribers.set(messageId, listeners);
      listeners.add(notify);
      return () => {
        listeners.delete(notify);
        if (!listeners.size) subscribers.delete(messageId);
      };
    },
    push(update: StreamUpdate) {
      const previous = pending?.messageId === update.messageId ? pending : delivered.get(update.messageId);
      if (previous && (previous.seq >= update.seq || previous.status !== "running")) return false;
      if (!previous || update.status !== "running") {
        cancelPending();
        publish(update);
      } else {
        pending = update;
        timer ??= window.setTimeout(() => {
          const next = pending;
          cancelPending();
          if (next) publish(next);
        }, paintIntervalMs);
      }
      return true;
    },
    reconcile(messages: readonly Message[]) {
      for (const message of messages) {
        if (message.status === "streaming") continue;
        forget(message.id);
      }
    },
    cancelPending,
  };
}

export type ChatStream = ReturnType<typeof createChatStream>;
