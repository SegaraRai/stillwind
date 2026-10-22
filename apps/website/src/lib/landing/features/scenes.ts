import { featureTopics } from "./examples";
import { clamp, ease } from "./progress";

const switchLength = 0.15;
let sceneStart = 0;

/** Example intervals in viewport heights, excluding the gap that switches source examples. */
export const featureScenes = featureTopics.flatMap((topic, scene) =>
  topic.frames.map((_, frame) => {
    const duration = scene === 0 ? 0.5 : scene === 2 ? 1.2 : frame === 2 ? 1.4 : 1;
    const start = sceneStart;
    const end = start + duration;
    sceneStart = end + switchLength;
    return {
      scene,
      frame,
      start,
      end,
    };
  }),
);

/** Total scroll distance in viewport heights. */
export const featureScrollLength = featureScenes.at(-1)!.end;

export const featureOffsets = featureTopics.map((_, scene) =>
  featureScenes.findIndex((stop) => stop.scene === scene),
);

/**
 * Maps finite scroll progress to an example and its independent 0..1 demonstration range.
 * Gaps switch source examples while holding the outgoing and incoming demonstrations at 1 and 0.
 * Only adjacent Syntax examples share tokens; other examples fade through an empty canvas.
 */
export function featurePosition(progress: number) {
  const absolute = clamp(progress) * featureScrollLength;
  const from = Math.max(
    0,
    featureScenes.findLastIndex((stop) => absolute >= stop.start),
  );
  const previous = featureScenes[from]!;
  const to = absolute > previous.end ? Math.min(from + 1, featureScenes.length - 1) : from;
  const next = featureScenes[to]!;
  const switching = from !== to;
  const gap = switching ? clamp((absolute - previous.end) / switchLength) : 0;
  const blend = ease(gap);
  const active = blend < 0.5 ? from : to;
  const current = featureScenes[active]!;
  const local = switching
    ? Number(active === from)
    : clamp((absolute - current.start) / (current.end - current.start));
  const morphing = switching && previous.scene === 0 && next.scene === 0;
  const syntax =
    previous.scene === 0
      ? (previous.frame + (morphing ? 0.3 + gap * 0.4 : 0)) / (featureTopics[0].frames.length - 1)
      : 1;
  return {
    from,
    to,
    blend,
    active,
    scene: current.scene,
    frame: current.frame,
    local,
    syntax,
    focusOpacity: morphing ? Math.abs(1 - blend * 2) : 1,
    opacity: featureTopics.map((_, scene) =>
      scene === current.scene ? (switching && !morphing ? Math.abs(1 - blend * 2) : 1) : 0,
    ),
  };
}
