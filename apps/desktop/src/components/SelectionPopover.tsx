import { BookmarkPlus, MessageSquareQuote } from "lucide-react";
import { useCallback, useEffect, useState, type RefObject } from "react";
import { createPortal } from "react-dom";

interface PopoverState {
  text: string;
  x: number;
  y: number;
  range: Range;
}

export function SelectionPopover({
  containerRef,
  disabled,
  onAsk,
  onSaveCard,
  onSelectionRange,
}: {
  containerRef: RefObject<HTMLElement | null>;
  disabled?: boolean;
  onAsk: (text: string, range: Range) => void;
  onSaveCard: (text: string, range: Range) => void;
  onSelectionRange?: (range: Range) => void;
}) {
  const [state, setState] = useState<PopoverState | null>(null);

  const evaluateSelection = useCallback(() => {
    if (disabled) {
      setState(null);
      return;
    }
    const container = containerRef.current;
    const selection = window.getSelection();
    if (!container || !selection || selection.isCollapsed || selection.rangeCount === 0) {
      setState(null);
      return;
    }
    const anchor = selection.anchorNode;
    const focus = selection.focusNode;
    if (!anchor || !focus || !container.contains(anchor) || !container.contains(focus)) {
      setState(null);
      return;
    }
    const anchorEl = anchor instanceof HTMLElement ? anchor : anchor.parentElement;
    if (anchorEl?.closest("button, input, textarea, a")) {
      setState(null);
      return;
    }
    const raw = selection.toString().trim();
    const charCount = Array.from(raw).length;
    if (charCount < 4 || charCount > 8000) {
      setState(null);
      return;
    }
    const range = selection.getRangeAt(0).cloneRange();
    const rect = range.getBoundingClientRect();
    if (rect.width === 0 && rect.height === 0) {
      setState(null);
      return;
    }
    onSelectionRange?.(range);
    const pillWidth = 228;
    const clampedX = Math.min(
      Math.max(rect.left + rect.width / 2, pillWidth / 2 + 12),
      window.innerWidth - pillWidth / 2 - 12,
    );
    const placeBelow = rect.top < 54;
    const y = placeBelow ? rect.bottom + 8 : rect.top - 8;
    setState({ text: raw, x: clampedX, y, range });
  }, [containerRef, disabled, onSelectionRange]);

  useEffect(() => {
    if (disabled) {
      setState(null);
      return;
    }
    const onMouseUp = () => {
      window.requestAnimationFrame(evaluateSelection);
    };
    const onKeyUp = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setState(null);
        return;
      }
      window.requestAnimationFrame(evaluateSelection);
    };
    const onSelectionChange = () => {
      const sel = window.getSelection();
      if (!sel || sel.isCollapsed) setState(null);
    };
    const onScrollOrResize = () => setState(null);

    document.addEventListener("mouseup", onMouseUp);
    document.addEventListener("keyup", onKeyUp);
    document.addEventListener("selectionchange", onSelectionChange);
    window.addEventListener("scroll", onScrollOrResize, true);
    window.addEventListener("resize", onScrollOrResize);
    return () => {
      document.removeEventListener("mouseup", onMouseUp);
      document.removeEventListener("keyup", onKeyUp);
      document.removeEventListener("selectionchange", onSelectionChange);
      window.removeEventListener("scroll", onScrollOrResize, true);
      window.removeEventListener("resize", onScrollOrResize);
    };
  }, [disabled, evaluateSelection]);

  if (!state) return null;

  const placeBelow = state.y > state.range.getBoundingClientRect().top;

  return createPortal(
    <div
      className="selection-popover"
      role="toolbar"
      aria-label="选中文字快捷操作"
      style={{
        position: "fixed",
        left: `${state.x}px`,
        top: `${state.y}px`,
        transform: placeBelow ? "translate(-50%, 0)" : "translate(-50%, -100%)",
        zIndex: 1200,
        display: "inline-flex",
        alignItems: "center",
        gap: "4px",
        padding: "4px 6px",
        borderRadius: "999px",
        background: "var(--surface-elevated, var(--surface))",
        border: "1px solid var(--line-strong, var(--line))",
        boxShadow: "0 8px 24px rgba(0, 0, 0, 0.16)",
      }}
      onMouseDown={(event) => {
        // Keep the active browser selection intact when clicking popover actions.
        event.preventDefault();
        event.stopPropagation();
      }}
    >
      <button
        type="button"
        className="button text-button"
        style={{ padding: "4px 10px", fontSize: "12.5px", borderRadius: "999px", gap: "5px" }}
        onClick={() => {
          const { text, range } = state;
          window.getSelection()?.removeAllRanges();
          setState(null);
          onAsk(text, range);
        }}
      >
        <MessageSquareQuote size={14} aria-hidden="true" />
        追问这段
      </button>
      <span aria-hidden="true" style={{ width: "1px", height: "14px", background: "var(--line)" }} />
      <button
        type="button"
        className="button text-button"
        style={{ padding: "4px 10px", fontSize: "12.5px", borderRadius: "999px", gap: "5px" }}
        onClick={() => {
          const { text, range } = state;
          window.getSelection()?.removeAllRanges();
          setState(null);
          onSaveCard(text, range);
        }}
      >
        <BookmarkPlus size={14} aria-hidden="true" />
        存为知识卡
      </button>
    </div>,
    document.body,
  );
}
