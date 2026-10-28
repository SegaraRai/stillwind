import { style } from "stillwind/react";

/**
 * Resizes an adjacent pane by dragging, or by arrow keys while focused.
 * The sash covers the divider between two grid tracks and must be placed in a positioned parent.
 *
 * @param props.orientation - `vertical` resizes widths, and `horizontal` resizes heights.
 * @param props.value - Current pane size in CSS pixels.
 * @param props.direction - `1` when moving the pointer right or down grows the pane, and `-1`
 *   when it shrinks the pane.
 * @param props.onChange - Receives the new size clamped between `min` and `max`.
 */
export function Sash({
  label,
  orientation,
  value,
  min,
  max,
  direction,
  onChange,
}: {
  label: string;
  orientation: "vertical" | "horizontal";
  value: number;
  min: number;
  max: number;
  direction: 1 | -1;
  onChange: (value: number) => void;
}) {
  const clamp = (next: number) => Math.round(Math.min(max, Math.max(min, next)));
  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={orientation}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      data-orientation={orientation}
      onPointerDown={(event) => {
        const handle = event.currentTarget;
        const start = orientation === "vertical" ? event.clientX : event.clientY;
        const initial = value;
        handle.setPointerCapture(event.pointerId);
        const move = (moved: PointerEvent) => {
          const current = orientation === "vertical" ? moved.clientX : moved.clientY;
          onChange(clamp(initial + (current - start) * direction));
        };
        const end = () => {
          handle.removeEventListener("pointermove", move);
          handle.removeEventListener("pointerup", end);
          handle.removeEventListener("pointercancel", end);
        };
        handle.addEventListener("pointermove", move);
        handle.addEventListener("pointerup", end);
        handle.addEventListener("pointercancel", end);
      }}
      onKeyDown={(event) => {
        const step = event.shiftKey ? 64 : 16;
        const grow = orientation === "vertical" ? "ArrowRight" : "ArrowDown";
        const shrink = orientation === "vertical" ? "ArrowLeft" : "ArrowUp";
        if (event.key === grow || event.key === shrink) {
          event.preventDefault();
          onChange(clamp(value + (event.key === grow ? step : -step) * direction));
        }
      }}
      {...style(
        "absolute z-10 touch-none [transition:background-color_150ms] max-workbench:hidden",
        "hover:bg-accent hover:[transition:background-color_150ms_300ms] focus-visible:bg-accent focus-visible:outline-none active:bg-accent",
        {
          "data-[orientation=vertical]:": "inset-y-0 -inset-s-0.5 inline-1 cursor-col-resize",
          "data-[orientation=horizontal]:": "inset-x-0 -inset-bs-0.5 block-1 cursor-row-resize",
        },
      )}
    />
  );
}
