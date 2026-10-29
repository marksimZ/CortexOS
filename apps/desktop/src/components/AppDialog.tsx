import { useImperativeHandle, useLayoutEffect, useRef, type ComponentPropsWithRef } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";

// Content is modal; the application titlebar remains available for window operations.
const dialogs: { element: HTMLDialogElement; returnTo: Element | null | (() => Element | null) }[] = [];
const previousInert = new Map<HTMLElement, boolean>();
let backdrop: HTMLDivElement | null = null;
const top = () => dialogs.at(-1)?.element;
const focusable = (root: Element) => Array.from(root.querySelectorAll<HTMLElement>('button,a[href],input,select,textarea,summary,[tabindex]:not([tabindex="-1"])'))
  .filter(e => !e.matches(":disabled") && !e.closest("[inert]") && e.getClientRects().length > 0);
function cancel(dialog: HTMLDialogElement) {
  if (dialog.dispatchEvent(new Event("cancel", { cancelable: true }))) dialog.close();
}
function focusDialog() {
  const dialog = top();
  if (dialog) (dialog.querySelector<HTMLElement>("[autofocus]") ?? focusable(dialog)[0] ?? dialog).focus({ preventScroll: true });
}
function keyboard(event: KeyboardEvent) {
  const dialog = top();
  if (!dialog || event.defaultPrevented) return;
  if (event.key === "Escape") {
    if (document.querySelector(":popover-open")) return;
    event.preventDefault(); cancel(dialog);
  } else if (event.key === "Tab") {
    const header = document.querySelector(".prototype-topbar");
    const targets = [...focusable(dialog), ...(header ? focusable(header) : [])];
    if (!targets.length) { event.preventDefault(); dialog.focus(); return; }
    const index = targets.indexOf(document.activeElement as HTMLElement);
    event.preventDefault();
    targets[(index + (event.shiftKey ? -1 : 1) + targets.length) % targets.length].focus();
  }
}
function keepFocus(event: FocusEvent) {
  const target = event.target;
  if (top() && target instanceof Element && !top()!.contains(target) && !target.closest(".prototype-topbar")) focusDialog();
}
function sync() {
  const active = top();
  document.body.classList.toggle("app-dialog-open", !!active);
  if (active) {
    if (!backdrop) {
      backdrop = document.createElement("div");
      backdrop.className = "app-dialog-backdrop";
      backdrop.setAttribute("aria-hidden", "true");
      document.body.append(backdrop);
      document.addEventListener("keydown", keyboard);
      document.addEventListener("focusin", keepFocus);
    }
    document.querySelectorAll<HTMLElement>(".prototype-sidebar,#main-content,.skip-link").forEach(e => {
      if (!previousInert.has(e)) previousInert.set(e, e.inert);
      e.inert = true;
    });
    dialogs.forEach(({ element }, index) => {
      element.inert = element !== active;
      element.parentElement?.style.setProperty("--dialog-order", String(index));
    });
  } else {
    backdrop?.remove(); backdrop = null;
    previousInert.forEach((inert, element) => { element.inert = inert; }); previousInert.clear();
    document.removeEventListener("keydown", keyboard);
    document.removeEventListener("focusin", keepFocus);
  }
}
function remove(dialog: HTMLDialogElement) {
  const index = dialogs.findIndex(item => item.element === dialog);
  if (index < 0) return;
  const wasTop = index === dialogs.length - 1;
  const [{ returnTo }] = dialogs.splice(index, 1);
  dialog.inert = false;
  sync();
  if (wasTop) {
    const target = typeof returnTo === "function" ? returnTo() : returnTo;
    if (target instanceof HTMLElement && target.isConnected && !target.closest("[inert]")) target.focus({ preventScroll: true });
    else focusDialog();
  }
}
export function openDialog(dialog: HTMLDialogElement | null, focusReturn?: () => Element | null) {
  if (!dialog || (dialog.open && dialogs.some(item => item.element === dialog))) return;
  const returnTo = focusReturn ?? document.activeElement;
  // React StrictMode can replay effects after removing a still-open registration.
  if (!dialog.open) dialog.show();
  dialogs.push({ element: dialog, returnTo }); sync();
  focusDialog();
}
export function AppDialog({ ref, children, className = "", onClose, ...props }: ComponentPropsWithRef<"dialog">) {
  const element = useRef<HTMLDialogElement>(null);
  useImperativeHandle(ref, () => element.current!);
  useLayoutEffect(() => {
    const dialog = element.current!;
    return () => remove(dialog);
  }, []);
  const ownHeader = className.includes("panel-dialog") || className.includes("evidence-overlay");
  return createPortal(<div className="app-dialog-layer"><dialog {...props} ref={element} className={className} aria-modal="false" tabIndex={-1}
    onClose={event => { remove(event.currentTarget); onClose?.(event); }}>
    {!ownHeader && <button type="button" className="icon-button app-dialog-close" aria-label="关闭弹窗" title="关闭" onClick={() => { if (element.current) cancel(element.current); }}><X size={18} aria-hidden="true" /></button>}
    {children}
  </dialog></div>, document.body);
}
