import { refA, refB } from "./barrel.js";

declare const enabled: boolean;

function identity<T>(value: T): T {
  return value;
}

const selected = enabled ? refA : refB;
export const probe_direct = selected;
export const probe_generic = identity(selected);
const wrapped = { current: selected } as const;
export const probe_object = wrapped.current;
const tuple = [refA, refB] as const;
export const probe_array = tuple[enabled ? 0 : 1];

function throughControlFlow(kind: "a" | "b") {
  let current = selected;
  if (kind === "a") {
    current = refA;
  }
  return current;
}

export const probe_control_flow = throughControlFlow(enabled ? "a" : "b");
