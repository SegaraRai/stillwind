/** Keeps the header in document flow at the top and reveals it above the demonstrations. */
export function mountHeader(slot: HTMLElement, story: HTMLElement): () => void {
  const view = slot.ownerDocument.defaultView!;
  const header = slot.querySelector<HTMLElement>("[data-site-header]")!;
  const abort = new view.AbortController();
  let frame: number | undefined;

  const update = () => {
    frame = undefined;
    const floating = slot.getBoundingClientRect().bottom <= 0;
    const visible = !floating || story.getBoundingClientRect().top <= 0;
    header.toggleAttribute("data-floating", floating);
    header.toggleAttribute("data-visible", visible);
    header.inert = !visible;
  };
  const schedule = () => {
    frame ??= view.requestAnimationFrame(update);
  };
  view.addEventListener("scroll", schedule, { passive: true, signal: abort.signal });
  view.addEventListener("resize", schedule, { passive: true, signal: abort.signal });
  slot.ownerDocument.fonts.addEventListener("loadingdone", schedule, { signal: abort.signal });
  const observer = new view.ResizeObserver(schedule);
  observer.observe(story);
  slot.ownerDocument.documentElement.setAttribute("data-header-mounted", "");
  update();

  return () => {
    abort.abort();
    observer.disconnect();
    if (frame !== undefined) {
      view.cancelAnimationFrame(frame);
    }
    header.removeAttribute("data-floating");
    header.removeAttribute("data-visible");
    header.inert = false;
    slot.ownerDocument.documentElement.removeAttribute("data-header-mounted");
  };
}
