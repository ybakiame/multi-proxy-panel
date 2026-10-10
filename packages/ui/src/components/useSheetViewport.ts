import { useEffect, useState } from "react";

/** 将抽屉约束在键盘上方的可见视口，并只滚动当前抽屉内的表单区域。 */
export function useSheetViewport(open: boolean) {
  const [container, setContainer] = useState<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!open || !container) return;
    const viewport = window.visualViewport;
    let frame = 0;
    const keepFocusedFieldVisible = () => {
      const field = document.activeElement;
      if (!(field instanceof HTMLElement) || !container.contains(field)) return;
      if (!field.matches("input, textarea, [contenteditable='true']")) return;
      const scroller = field.closest<HTMLElement>("[data-sheet-scroll]");
      if (!scroller || !container.contains(scroller)) return;
      const fieldRect = field.getBoundingClientRect();
      const scrollRect = scroller.getBoundingClientRect();
      const top = Math.max(scrollRect.top, viewport?.offsetTop ?? 0) + 12;
      const bottom = Math.min(scrollRect.bottom, (viewport?.offsetTop ?? 0) + (viewport?.height ?? innerHeight)) - 12;
      if (fieldRect.bottom > bottom) scroller.scrollTop += fieldRect.bottom - bottom;
      else if (fieldRect.top < top) scroller.scrollTop -= top - fieldRect.top;
    };
    const update = () => {
      const height = viewport?.height ?? window.innerHeight;
      const bottom = Math.max(0, window.innerHeight - height - (viewport?.offsetTop ?? 0));
      container.style.setProperty("--sheet-viewport-height", `${height}px`);
      container.style.setProperty("--sheet-viewport-bottom", `${bottom}px`);
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(keepFocusedFieldVisible);
    };
    update();
    viewport?.addEventListener("resize", update);
    viewport?.addEventListener("scroll", update);
    window.addEventListener("resize", update);
    container.addEventListener("focusin", update);
    return () => {
      cancelAnimationFrame(frame);
      viewport?.removeEventListener("resize", update);
      viewport?.removeEventListener("scroll", update);
      window.removeEventListener("resize", update);
      container.removeEventListener("focusin", update);
    };
  }, [open, container]);
  return setContainer;
}
