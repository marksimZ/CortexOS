import { useEffect, useState } from "react";

function prefersReducedMotion(): boolean {
  return typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/**
 * Keeps a conditionally rendered element mounted briefly while `closing` is true
 * so CSS exit animations can play before DOM removal.
 */
export function useExitPresence(open: boolean, durationMs = 150): { mounted: boolean; closing: boolean } {
  const [mounted, setMounted] = useState(open);
  const [closing, setClosing] = useState(false);

  if (open && (!mounted || closing)) {
    setMounted(true);
    setClosing(false);
  }

  useEffect(() => {
    if (open) return;
    if (!mounted) return;
    if (prefersReducedMotion()) {
      setMounted(false);
      setClosing(false);
      return;
    }
    setClosing(true);
    const timer = window.setTimeout(() => {
      setMounted(false);
      setClosing(false);
    }, durationMs);
    return () => window.clearTimeout(timer);
  }, [open, mounted, durationMs]);

  return { mounted: open || mounted, closing: !open && closing };
}

/**
 * Retains the last non-null value during the exit animation window so
 * coordinate-positioned menus/popovers can animate out in place.
 */
export function useStickyPresence<T>(value: T | null, durationMs = 150): { current: T | null; closing: boolean } {
  const [cached, setCached] = useState<T | null>(value);
  const [closing, setClosing] = useState(false);

  if (value !== null && (cached !== value || closing)) {
    setCached(value);
    setClosing(false);
  }

  useEffect(() => {
    if (value !== null) return;
    if (cached === null) return;
    if (prefersReducedMotion()) {
      setCached(null);
      setClosing(false);
      return;
    }
    setClosing(true);
    const timer = window.setTimeout(() => {
      setCached(null);
      setClosing(false);
    }, durationMs);
    return () => window.clearTimeout(timer);
  }, [value, cached, durationMs]);

  return { current: value ?? cached, closing: value === null && cached !== null && closing };
}
