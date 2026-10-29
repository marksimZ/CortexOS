import { useEffect, useRef, useState, type CSSProperties, type RefObject } from "react";

export function splitStyle(ratio: number | null, gap: number): CSSProperties | undefined {
  return ratio === null ? undefined : {
    "--split-left": `calc(${ratio * 100}% - ${gap * ratio}px)`,
    "--split-right": "1fr",
    "--split-handle-left": `calc(${ratio * 100}% + ${gap * (0.5 - ratio)}px)`,
  } as CSSProperties;
}

export function PaneDivider({ root, ratio, onChange, onCollapse, label }: {
  root: RefObject<HTMLDivElement | null>;
  ratio: number | null;
  onChange: (ratio: number | null) => void;
  onCollapse: () => void;
  label: string;
}) {
  const handle = useRef<HTMLDivElement>(null);
  const drag = useRef<{ initial: number | null; pointer: number; beyond: boolean } | null>(null);
  const [dragging, setDragging] = useState(false);
  const [measured, setMeasured] = useState(0.5);
  useEffect(() => {
    const element = root.current;
    if (!element) return;
    const observer = new ResizeObserver(() => {
      const first = element.firstElementChild;
      const gap = parseFloat(getComputedStyle(element).columnGap) || 0;
      const scale = element.getBoundingClientRect().width / Math.max(1, element.offsetWidth);
      if (first) setMeasured(first.getBoundingClientRect().width / Math.max(1, element.getBoundingClientRect().width - gap * scale));
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [root]);
  useEffect(() => () => {
    const current = drag.current;
    drag.current = null;
    if (current) onChange(current.initial);
  }, []);
  const finish = (cancel: boolean) => {
    const current = drag.current;
    if (!current) return;
    drag.current = null;
    if (cancel || current.beyond) onChange(current.initial);
    if (!cancel && current.beyond) onCollapse();
    if (handle.current?.hasPointerCapture(current.pointer)) handle.current.releasePointerCapture(current.pointer);
    setDragging(false);
  };
  useEffect(() => {
    if (!dragging) return;
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault(); event.stopPropagation(); finish(true);
    };
    const blur = () => finish(true);
    window.addEventListener("keydown", escape, true);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("keydown", escape, true);
      window.removeEventListener("blur", blur);
    };
  }, [dragging]);
  const adjust = (value: number) => {
    if (value > 0.8) { onCollapse(); return; }
    onChange(Math.max(0.2, Math.min(0.8, value)));
  };
  return <>
    <div ref={handle} role="separator" tabIndex={0} aria-label={label} aria-orientation="vertical"
      aria-valuemin={20} aria-valuemax={80} aria-valuenow={Math.round((ratio ?? measured) * 100)}
      aria-valuetext={`左侧 ${Math.round((ratio ?? measured) * 100)}%`}
      title="拖动调整，双击均分；方向键调整，Esc 取消拖动" className="pane-divider"
      onPointerDown={event => {
        if (event.button !== 0) return;
        event.preventDefault(); event.currentTarget.focus({ preventScroll: true });
        drag.current = { initial: ratio, pointer: event.pointerId, beyond: false };
        event.currentTarget.setPointerCapture(event.pointerId); setDragging(true);
      }}
      onPointerMove={event => {
        if (!drag.current || drag.current.pointer !== event.pointerId || !root.current) return;
        const bounds = root.current.getBoundingClientRect();
        const gap = parseFloat(getComputedStyle(root.current).columnGap) || 0;
        const physicalGap = gap * bounds.width / Math.max(1, root.current.offsetWidth);
        const value = (event.clientX - bounds.left - physicalGap / 2) / Math.max(1, bounds.width - physicalGap);
        drag.current.beyond = value > 0.8;
        onChange(Math.max(0.2, Math.min(0.8, value)));
      }}
      onPointerUp={() => finish(false)} onPointerCancel={() => finish(true)} onLostPointerCapture={() => finish(true)}
      onDoubleClick={() => onChange(0.5)}
      onKeyDown={event => {
        if (["ArrowLeft", "ArrowRight", "Home", "End", "Enter"].includes(event.key)) {
          event.preventDefault();
          adjust(event.key === "Home" ? 0.2 : event.key === "End" ? 0.8 : event.key === "Enter" ? 0.5 : (ratio ?? measured) + (event.key === "ArrowLeft" ? -0.025 : 0.025));
        }
      }} />
    {dragging && <div className="pane-drag-shield" aria-hidden="true" />}
  </>;
}
