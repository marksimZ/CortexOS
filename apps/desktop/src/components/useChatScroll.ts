import { useEffect, useLayoutEffect, useRef, useState } from "react";

export function useChatScroll(conversation: string | null, messageCount: number, content: unknown, located: boolean) {
  const scroller = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  const upwardIntent = useRef(false);
  const previousTop = useRef(0);
  const touchY = useRef(0);
  const [showJump, setShowJump] = useState(false);
  const metrics = () => {
    const element = scroller.current!;
    const text = element.querySelector(".message:last-child .message-content") ?? element;
    const style = getComputedStyle(text);
    const line = parseFloat(style.lineHeight) || parseFloat(style.fontSize) * 1.5;
    return { element, line, distance: Math.max(0, element.scrollHeight - element.clientHeight - element.scrollTop) };
  };
  const refresh = () => {
    if (!scroller.current) return;
    const { element, line } = metrics();
    if (following.current && !located) {
      element.scrollTop = element.scrollHeight;
      previousTop.current = element.scrollTop;
    }
    setShowJump(metrics().distance > line * 3);
  };
  useLayoutEffect(() => {
    following.current = !located;
    upwardIntent.current = false;
    previousTop.current = 0;
    setShowJump(false);
  }, [conversation, located]);
  useLayoutEffect(refresh, [content, conversation, located]);
  useEffect(() => {
    const element = scroller.current;
    if (!element) return;
    const observer = new ResizeObserver(refresh);
    observer.observe(element);
    const messages = element.querySelector(".messages");
    if (messages) observer.observe(messages);
    return () => observer.disconnect();
  }, [conversation, messageCount, located]);
  return {
    scroller, showJump,
    onScroll: () => {
      const { element, distance, line } = metrics();
      if (element.scrollTop < previousTop.current) following.current = false;
      else if (!upwardIntent.current && element.scrollTop > previousTop.current && distance < line) following.current = true;
      previousTop.current = element.scrollTop;
      setShowJump(distance > line * 3);
    },
    onWheel: (event: React.WheelEvent) => {
      if (event.deltaY === 0) return;
      upwardIntent.current = event.deltaY < 0;
      if (upwardIntent.current) following.current = false;
    },
    onPointerDown: () => { upwardIntent.current = false; },
    onTouchStart: (event: React.TouchEvent) => { touchY.current = event.touches[0]?.clientY ?? 0; },
    onTouchMove: (event: React.TouchEvent) => {
      const y = event.touches[0]?.clientY ?? touchY.current;
      if (y !== touchY.current) upwardIntent.current = y > touchY.current;
      if (upwardIntent.current) following.current = false;
      touchY.current = y;
    },
    onKeyDown: (event: React.KeyboardEvent) => {
      if (["ArrowUp", "PageUp", "Home"].includes(event.key) || (event.key === " " && event.shiftKey)) {
        upwardIntent.current = true; following.current = false;
      } else if (["ArrowDown", "PageDown", "End"].includes(event.key) || event.key === " ") upwardIntent.current = false;
    },
    jump: () => { upwardIntent.current = false; following.current = true; refresh(); },
  };
}
