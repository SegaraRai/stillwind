import { applyStyle, style } from "stillwind/vanilla";

import { createCodeMorph } from "../code-morph";
import { featureTopics } from "./examples";
import { featureOffsets, featureScenes, type featurePosition } from "./scenes";

/** Controls source visibility, navigation labels, highlights, and measured connectors. */
export function createFeaturePresentation(root: HTMLElement, signal: AbortSignal) {
  const view = root.ownerDocument.defaultView!;
  const stacked = view.matchMedia(style.media("max-sidebar"));
  const panels = [...root.querySelectorAll<HTMLElement>("[data-feature-panel]")];
  const steps = [...root.querySelectorAll<HTMLAnchorElement>("[data-feature-step]")];
  const frames = panels.map((panel) => [
    ...panel.querySelectorAll<HTMLElement>("[data-feature-frame]"),
  ]);
  const previews = panels.map((panel) => [
    ...panel.querySelectorAll<HTMLElement>("[data-feature-preview-frame]"),
  ]);
  const captions = panels.map((panel) =>
    panel.querySelector<HTMLElement>("[data-feature-caption]")!,
  );
  const descriptions = panels.map((panel) =>
    panel.querySelector<HTMLElement>("[data-feature-description]")!,
  );
  const counters = panels.map((panel) =>
    panel.querySelector<HTMLElement>("[data-feature-counter]")!,
  );
  const cues = [...root.querySelectorAll<HTMLButtonElement>("[data-feature-cue]")];
  const paging = [...root.querySelectorAll<HTMLButtonElement>("[data-feature-direction]")];
  const syntaxButtons = [...root.querySelectorAll<HTMLButtonElement>("[data-syntax-button]")];
  const syntaxMorph = createCodeMorph(
    panels[0]!.querySelector<HTMLElement>("[data-feature-source]")!,
    panels[0]!,
  );

  syntaxButtons.forEach((button, index) => {
    button.disabled = index < 3;
    if (index >= 3) {
      button.addEventListener(
        "click",
        () => {
          const pressed = button.getAttribute("aria-pressed") !== "true";
          button.setAttribute("aria-pressed", String(pressed));
          button
            .closest("[data-feature-preview-frame]")!
            .querySelector("[data-syntax-state]")!.textContent =
            'aria-pressed="' + String(pressed) + '"';
        },
        { signal },
      );
    }
  });

  return {
    setActive(scene: number, frame: number, enhanced: boolean) {
      panels.forEach((panel, index) => {
        const selected = !enhanced || index === scene;
        panel.inert = !selected;
        panel.toggleAttribute("data-active", selected);
        if (selected) {
          panel.removeAttribute("aria-hidden");
        } else {
          panel.setAttribute("aria-hidden", "true");
        }
        const localFrame = index === scene ? frame : index < scene ? frames[index]!.length - 1 : 0;
        frames[index]!.forEach((source, sourceIndex) => {
          const visible = !enhanced || sourceIndex === localFrame;
          source.toggleAttribute("data-current", visible);
          source
            .closest<HTMLElement>("[data-feature-example]")!
            .toggleAttribute("data-current", visible);
          source.setAttribute("aria-hidden", String(enhanced && sourceIndex !== localFrame));
        });
        previews[index]!.forEach((preview, previewIndex) => {
          const visible = !enhanced || previewIndex === localFrame;
          preview.toggleAttribute("data-current", visible);
          preview.inert = !visible;
          preview.setAttribute("aria-hidden", String(!visible));
        });
        const specimen = featureTopics[index]!.frames[localFrame]!;
        captions[index]!.textContent = specimen.label;
        descriptions[index]!.textContent = specimen.description;
        counters[index]!.textContent =
          String(localFrame + 1).padStart(2, "0") +
          " / " +
          String(frames[index]!.length).padStart(2, "0");
      });
      steps.forEach((step, index) => {
        if (enhanced && index === scene) {
          step.setAttribute("aria-current", "step");
        } else {
          step.removeAttribute("aria-current");
        }
      });
      cues.forEach((cue, index) => {
        const selected = index === featureOffsets[scene]! + frame;
        cue.toggleAttribute("data-current", selected);
        cue.setAttribute("aria-pressed", String(selected));
        cue.hidden = featureScenes[index]!.scene !== scene;
      });
      paging.forEach((button) => {
        const next = featureOffsets[scene]! + frame + Number(button.dataset.featureDirection);
        button.disabled = next < 0 || next >= featureScenes.length;
      });
    },
    render(current: ReturnType<typeof featurePosition>, compact: boolean) {
      panels.forEach((panel, index) => {
        const opacity =
          compact && current.scene === 0 && index === 0 && current.from !== current.to
            ? Math.abs(1 - current.blend * 2)
            : current.opacity[index]!;
        panel.style.setProperty("--feature-opacity", opacity.toFixed(5));
      });
      if (!compact) {
        syntaxMorph.render(current.syntax);
      }
    },
    measureSizes(compact: boolean) {
      if (compact) {
        panels.forEach((panel, index) => {
          const source = panel.querySelector<HTMLElement>("[data-feature-source]")!;
          source.style.setProperty(
            "--code-height",
            Math.max(...frames[index]!.map((frame) => frame.getBoundingClientRect().height)) + "px",
          );
          source.style.setProperty(
            "--preview-height",
            Math.max(...previews[index]!.map((preview) => preview.getBoundingClientRect().height)) +
              "px",
          );
        });
      }
    },
    measure(scene: number, frameIndex: number, focusOpacity: number) {
      const panel = panels[scene];
      if (!panel) {
        return;
      }
      const canvas = panel.querySelector<HTMLElement>("[data-code-canvas]")!;
      const bounds = canvas.getBoundingClientRect();
      const source = panel.querySelector<HTMLElement>("[data-feature-source]")!;
      const sourceBounds = source.getBoundingClientRect();
      const frame = frames[scene]![frameIndex]!;
      const origins = [...frame.querySelectorAll<HTMLElement>("[data-feature-origin]")];
      const rects: {
        left: number;
        right: number;
        top: number;
        bottom: number;
        width: number;
        height: number;
      }[] = [];
      for (const rect of origins.flatMap((origin) => [...origin.getClientRects()])) {
        if (!rect.width) {
          continue;
        }
        const row = rects.find((line) => Math.abs(line.top - rect.top) < 1);
        if (row) {
          row.left = Math.min(row.left, rect.left);
          row.right = Math.max(row.right, rect.right);
          row.bottom = Math.max(row.bottom, rect.bottom);
          row.width = row.right - row.left;
          row.height = row.bottom - row.top;
        } else {
          rects.push({
            left: rect.left,
            right: rect.right,
            top: rect.top,
            bottom: rect.bottom,
            width: rect.width,
            height: rect.height,
          });
        }
      }
      const last = rects.at(-1) ?? origins[0]!.getBoundingClientRect();
      const highlights = source.querySelector<HTMLElement>("[data-feature-highlights]")!;
      highlights.style.setProperty("--focus-opacity", String(focusOpacity));
      highlights.replaceChildren(
        ...rects.map((rect) => {
          const highlight = root.ownerDocument.createElement("span");
          applyStyle(
            highlight,
            style(
              "inset-s-(--highlight-x) inset-bs-(--highlight-y) block-(--highlight-height) inline-(--highlight-width) source-highlight",
            ),
          );
          highlight.style.setProperty("--highlight-x", rect.left - sourceBounds.left + "px");
          highlight.style.setProperty("--highlight-y", rect.top - sourceBounds.top + "px");
          highlight.style.setProperty("--highlight-width", rect.width + "px");
          highlight.style.setProperty("--highlight-height", rect.height + 2 + "px");
          return highlight;
        }),
      );
      const preview = previews[scene]![Math.min(frameIndex, previews[scene]!.length - 1)]!;
      const target = (
        preview.querySelector<HTMLElement>("[data-feature-target]") ?? preview
      ).getBoundingClientRect();
      const x1 =
        (Math.min(...rects.map((rect) => rect.left)) +
          Math.max(...rects.map((rect) => rect.right))) /
          2 -
        bounds.left;
      const y1 = last.bottom - bounds.top + 5;
      const x2 = target.left - bounds.left - 18;
      const y2 = target.top - bounds.top + target.height / 2;
      const line = panel.querySelector<SVGPathElement>("[data-feature-line]")!;
      line.style.setProperty("--focus-opacity", String(focusOpacity));
      if (stacked.matches) {
        const endX = target.left - bounds.left + target.width / 2;
        const endY = target.top - bounds.top - 12;
        const gutter = Math.min(
          bounds.width - 4,
          Math.max(
            x1,
            ...[...frame.querySelectorAll<HTMLElement>("[data-part]")]
              .filter((part) => part.textContent!.trim())
              .map((part) => part.getBoundingClientRect().right - bounds.left + 16),
          ),
        );
        const sourceBottom = frame.getBoundingClientRect().bottom - bounds.top + 10;
        line.setAttribute("d", `M ${x1} ${y1} H ${gutter} V ${sourceBottom} L ${endX} ${endY}`);
        return;
      }
      const rising = y2 < y1;
      const obstacles = [...frame.querySelectorAll<HTMLElement>("[data-part]")].filter((part) => {
        const rect = part.getBoundingClientRect();
        return (
          part.textContent!.trim() &&
          (rising ? rect.bottom >= target.top + target.height / 2 : rect.top > last.bottom)
        );
      });
      const gutter = Math.max(
        x1,
        ...obstacles.map((part) => part.getBoundingClientRect().right - bounds.left + 18),
      );
      line.setAttribute(
        "d",
        "M" +
          x1 +
          "," +
          y1 +
          " V" +
          (y1 + 6) +
          (obstacles.length ? " H" + gutter : "") +
          " L" +
          x2 +
          "," +
          y2,
      );
    },
    clearProgress() {
      previews.flat().forEach((preview) => preview.style.removeProperty("--demo-progress"));
    },
    dispose() {
      syntaxMorph.dispose();
      syntaxButtons.forEach((button) => {
        button.disabled = true;
      });
    },
  };
}
