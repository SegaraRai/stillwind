import { expect, inject, test } from "vite-plus/test";

import type { BrowserCssCase } from "./browser-setup.ts";

async function read(entry: BrowserCssCase, side: "expected" | "actual") {
  const frame = document.createElement("iframe");
  const ready = new Promise<void>((resolve) => {
    frame.onload = () => resolve();
  });
  frame.srcdoc = `<style>${entry[side]}${entry.extra ?? ""}</style><div id="parent"><div id="probe"></div></div>`;
  document.body.append(frame);
  try {
    await ready;
    const doc = frame.contentDocument!;
    const element = doc.getElementById("probe")!;
    doc.getElementById("parent")!.style.cssText = entry.parent ?? "";
    element.className = entry[`${side}Class`];
    for (const animation of element.getAnimations()) {
      animation.pause();
      animation.currentTime = 500;
    }
    const style = frame.contentWindow!.getComputedStyle(element);
    const values = entry.properties.map((name) => style.getPropertyValue(name));
    let pixels: number[] = [];
    if (entry.properties.includes("filter")) {
      const canvas = doc.createElement("canvas");
      canvas.width = 128;
      canvas.height = 128;
      const context = canvas.getContext("2d")!;
      context.filter = style.filter;
      context.fillStyle = "white";
      context.fillRect(48, 48, 24, 24);
      pixels = Array.from(context.getImageData(0, 0, 128, 128).data);
    }
    return { values, pixels };
  } finally {
    frame.remove();
  }
}

test.each(inject("tailwindCases"))("$name", async (entry) => {
  const expected = await read(entry, "expected");
  const actual = await read(entry, "actual");
  if (entry.different) {
    expect(expected.values[0]).toBe("1");
    expect(actual.values[0]).toBe("0.5");
  } else if (expected.pixels.length) {
    expect(actual.pixels).toHaveLength(expected.pixels.length);
    // Color conversion can round each 8-bit channel by one step.
    const maxPixelError = expected.pixels.reduce(
      (max, value, index) => Math.max(max, Math.abs(value - actual.pixels[index])),
      0,
    );
    expect(maxPixelError).toBeLessThanOrEqual(1);
  } else {
    expect(actual.values).toStrictEqual(expected.values);
  }
});
