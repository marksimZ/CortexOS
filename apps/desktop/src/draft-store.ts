import { useEffect, useRef } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

const prefix = "cortexos.draft.v1.";
const volatile = new Map<string, unknown>();
const editors = new Set<() => boolean>();
let closing = false;
let closeNative: (() => Promise<void>) | undefined;
let requestNativeClose: (() => Promise<void>) | undefined;

// The caller supplies a key scoped to the workspace and the document's base revision.
export function draftStore<T>(namespace: string, accepts: (value: unknown) => boolean = () => true) {
  const keyFor = (key: string) => `${prefix}${namespace}.${encodeURIComponent(key)}`;
  return {
    get(key: string): T | undefined {
      const storageKey = keyFor(key);
      try {
        if (volatile.has(storageKey)) return volatile.get(storageKey) as T | undefined;
        const raw = localStorage.getItem(storageKey);
        if (raw === null) return;
        const value: unknown = JSON.parse(raw);
        if (accepts(value)) return value as T;
      } catch { /* An unavailable or invalid local draft cannot replace saved data. */ }
    },
    has(key: string): boolean { return this.get(key) !== undefined; },
    persisted(key: string): boolean { return !volatile.has(keyFor(key)) && this.has(key); },
    set(key: string, value: T): boolean {
      const storageKey = keyFor(key);
      try {
        localStorage.setItem(storageKey, JSON.stringify(value));
        volatile.delete(storageKey);
        return true;
      } catch {
        volatile.set(storageKey, value);
        return false;
      }
    },
    delete(key: string) {
      const storageKey = keyFor(key);
      try {
        localStorage.removeItem(storageKey);
        volatile.delete(storageKey);
      } catch {
        // Remember the deletion for this session; a blocked storage backend may still hold the old draft.
        volatile.set(storageKey, undefined);
      }
    },
  };
}

export function registerDraftEditor(requestClose: () => boolean) {
  editors.add(requestClose);
  return () => { editors.delete(requestClose); };
}

export function requestDraftWindowClose() {
  return requestNativeClose?.() ?? getCurrentWindow().close();
}

export function closeDraftWindow() {
  return closeNative?.() ?? getCurrentWindow().close();
}

export function useDraftWindow(onVolatile: () => void, onError: (error: unknown) => void) {
  const callbacks = useRef({ onVolatile, onError });
  callbacks.current = { onVolatile, onError };
  useEffect(() => {
    let disposed = false;
    let listener: Promise<(() => void) | void> | undefined;
    const warn = (event: BeforeUnloadEvent) => {
      if (!closing && volatile.size) event.preventDefault();
    };
    window.addEventListener("beforeunload", warn);
    const native = isTauri() ? getCurrentWindow() : undefined;
    const report = (error: unknown) => { if (!disposed) callbacks.current.onError(error); };
    const request = async () => {
      if (disposed || closing) return;
      const editor = Array.from(editors).at(-1);
      if (editor?.()) return;
      if (volatile.size) callbacks.current.onVolatile();
      else await close();
    };
    const listen = () => native?.onCloseRequested(event => {
      event.preventDefault();
      void request().catch(report);
    }).catch(report);
    const close = async () => {
      if (!native || closing) return;
      closing = true;
      try {
        // Remove this guard before the allowed close command re-enters the native close path.
        await (await listener)?.();
        await native.close();
      } catch (error) {
        closing = false;
        if (!disposed) listener = listen();
        throw error;
      }
    };
    if (native) {
      listener = listen();
      closeNative = close;
      requestNativeClose = request;
    }
    return () => {
      disposed = true;
      if (closeNative === close) closeNative = undefined;
      if (requestNativeClose === request) requestNativeClose = undefined;
      window.removeEventListener("beforeunload", warn);
      void listener?.then(unlisten => unlisten?.()).catch(report);
    };
  }, []);
}
