import { describe, expect, it, vi } from "vite-plus/test";

import { scrollProgress, trackScroll } from "./scroll";

function browserRange() {
  const events = new EventTarget();
  const mediaEvents = new EventTarget();
  const media = Object.assign(mediaEvents, { matches: true });
  const properties = new Map<string, { value: string; priority: string }>();
  const frames = new Map<number, FrameRequestCallback>();
  let frameId = 0;
  let rect = { top: 0, height: 3000 };
  const disconnect = vi.fn();
  const view = {
    innerHeight: 1000,
    scrollY: 500,
    addEventListener: events.addEventListener.bind(events),
    removeEventListener: events.removeEventListener.bind(events),
    matchMedia: () => media,
    requestAnimationFrame(callback: FrameRequestCallback) {
      frames.set(++frameId, callback);
      return frameId;
    },
    cancelAnimationFrame(id: number) {
      frames.delete(id);
    },
    scrollTo: vi.fn(),
    ResizeObserver: class {
      observe() {}
      disconnect = disconnect;
    },
  };
  const element = {
    ownerDocument: { defaultView: view },
    getBoundingClientRect: vi.fn(() => rect),
    style: {
      getPropertyValue: (name: string) => properties.get(name)?.value ?? "",
      getPropertyPriority: (name: string) => properties.get(name)?.priority ?? "",
      setProperty(name: string, value: string, priority = "") {
        properties.set(name, { value, priority });
      },
      removeProperty(name: string) {
        properties.delete(name);
      },
    },
  };
  return {
    element,
    view,
    events,
    media,
    frames,
    disconnect,
    move(top: number) {
      rect = { ...rect, top };
      events.dispatchEvent(new Event("scroll"));
    },
    flush() {
      const pending = [...frames.values()];
      frames.clear();
      pending.forEach((callback) => callback(0));
    },
    connect(options: Parameters<typeof trackScroll>[1] = {}) {
      return trackScroll(element as unknown as HTMLElement, options);
    },
  };
}

describe("scroll progress", () => {
  it("uses the distance between entry and the end of the sticky range", () => {
    expect(scrollProgress(100, 3000, 1000)).toBe(0);
    expect(scrollProgress(-1000, 3000, 1000)).toBe(0.5);
    expect(scrollProgress(-3000, 3000, 1000)).toBe(1);
    expect(scrollProgress(0, 800, 1000)).toBe(0);
    expect(scrollProgress(-1, 800, 1000)).toBe(1);
  });

  it("coalesces events and publishes the latest geometry as a CSS variable", () => {
    const range = browserRange();
    const onProgress = vi.fn();
    const controller = range.connect({ onProgress });
    range.move(-500);
    range.move(-1000);
    range.events.dispatchEvent(new Event("resize"));
    expect(range.frames.size).toBe(1);
    range.flush();
    expect(range.element.getBoundingClientRect).toHaveBeenCalledTimes(1);
    expect(range.element.style.getPropertyValue("--scroll-progress")).toBe("0.50000");
    expect(onProgress).toHaveBeenLastCalledWith(0.5);
    controller.dispose();
  });

  it("removes publication while the media query disables tracking and resumes at current progress", () => {
    const range = browserRange();
    const onEnable = vi.fn();
    const controller = range.connect({
      media: "(prefers-reduced-motion: no-preference)",
      onEnable,
    });
    range.flush();
    range.media.matches = false;
    range.media.dispatchEvent(new Event("change"));
    expect(range.element.style.getPropertyValue("--scroll-progress")).toBe("");
    range.move(-1000);
    expect(range.frames.size).toBe(0);
    expect(onEnable).toHaveBeenLastCalledWith(false);
    range.media.matches = true;
    range.media.dispatchEvent(new Event("change"));
    range.flush();
    expect(range.element.style.getPropertyValue("--scroll-progress")).toBe("0.50000");
    expect(onEnable).toHaveBeenLastCalledWith(true);
    controller.dispose();
  });

  it("disposes pending work, restores the property, and releases every subscription", () => {
    const range = browserRange();
    range.element.style.setProperty("--position", "original", "important");
    const controller = range.connect({ property: "--position", media: "(min-width: 960px)" });
    range.move(-1000);
    range.flush();
    range.move(-1500);
    controller.dispose();
    controller.dispose();
    expect(range.frames.size).toBe(0);
    expect(range.disconnect).toHaveBeenCalledTimes(1);
    expect(range.element.style.getPropertyValue("--position")).toBe("original");
    expect(range.element.style.getPropertyPriority("--position")).toBe("important");
    range.move(-2000);
    range.events.dispatchEvent(new Event("resize"));
    range.media.dispatchEvent(new Event("change"));
    expect(range.frames.size).toBe(0);
  });

  it("navigates to an absolute position within the current range", () => {
    const range = browserRange();
    const controller = range.connect();
    range.move(-100);
    controller.scrollTo(0.5);
    expect(range.view.scrollTo).toHaveBeenLastCalledWith({ top: 1400, behavior: "auto" });
    controller.scrollTo(2);
    expect(range.view.scrollTo).toHaveBeenLastCalledWith({ top: 2400, behavior: "auto" });
    controller.scrollTo(1 / 3, "smooth");
    expect(range.view.scrollTo).toHaveBeenLastCalledWith({
      top: 400 + 2000 / 3,
      behavior: "smooth",
    });
    controller.dispose();
  });
});
