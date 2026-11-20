import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

export function useChatScroll(conversation: string | null, messages: readonly unknown[], located: boolean) {
  const scroller = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  const upwardIntent = useRef(false);
  const previousTop = useRef(0);
  const touchY = useRef(0);
  const line = useRef(24);
  const [showJump, setShowJump] = useState(false);

  const refresh = useCallback(() => {
    const element = scroller.current;
    if (!element) return;
    const text = element.querySelector(".message:last-child .message-content") ?? element;
    const style = getComputedStyle(text);
    line.current = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.5;
    const end = Math.max(0, element.scrollHeight - element.clientHeight);
    const top = element.scrollTop;
    if (following.current && !located) {
      if (Math.abs(end - top) >= 1) {
        element.scrollTop = end;
        // WebView2 may round or clamp the written position.
        previousTop.current = element.scrollTop;
      } else {
        previousTop.current = top;
      }
      setShowJump(false);
    } else {
      setShowJump(end - top > line.current * 3);
    }
  }, [located]);

  useLayoutEffect(() => {
    following.current = !located;
    upwardIntent.current = false;
    previousTop.current = 0;
    setShowJump(false);
  }, [conversation, located]);
  useLayoutEffect(refresh, [messages, conversation, refresh]);
  const hasMessages = messages.length > 0;
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    let frame: number | undefined;
    const observer = new ResizeObserver(() => {
      frame ??= requestAnimationFrame(() => {
        frame = undefined;
        refresh();
      });
    });
    observer.observe(element);
    const list = element.querySelector(".messages");
    if (list) observer.observe(list);
    return () => {
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [conversation, hasMessages, refresh]);

  const pause = () => { upwardIntent.current = true; following.current = false; };
  return {
    scroller, showJump, pause,
    onScroll: () => {
      const element = scroller.current;
      if (!element) return;
      const top = element.scrollTop;
      const distance = Math.max(0, element.scrollHeight - element.clientHeight - top);
      if (top < previousTop.current) following.current = false;
      else if (!upwardIntent.current && top > previousTop.current && distance < line.current) following.current = true;
      previousTop.current = top;
      setShowJump(distance > line.current * 3);
    },
    onWheel: (event: React.WheelEvent) => {
      if (event.deltaY < 0) pause();
      else if (event.deltaY > 0) upwardIntent.current = false;
    },
    onPointerDown: () => { upwardIntent.current = false; },
    onTouchStart: (event: React.TouchEvent) => { touchY.current = event.touches[0]?.clientY ?? 0; },
    onTouchMove: (event: React.TouchEvent) => {
      const y = event.touches[0]?.clientY ?? touchY.current;
      if (y > touchY.current) pause();
      else if (y < touchY.current) upwardIntent.current = false;
      touchY.current = y;
    },
    onKeyDown: (event: React.KeyboardEvent) => {
      if (["ArrowUp", "PageUp", "Home"].includes(event.key) || (event.key === " " && event.shiftKey)) pause();
      else if (["ArrowDown", "PageDown", "End"].includes(event.key) || event.key === " ") upwardIntent.current = false;
    },
    jump: () => { upwardIntent.current = false; following.current = true; refresh(); },
  };
}
