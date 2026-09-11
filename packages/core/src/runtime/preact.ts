import { createFragmentRuntime } from "./fragment.ts";

export * from "./shared.ts";

const runtime = createFragmentRuntime({
  classProperty: "class",
  styleFormat: "object",
});

export const stillwindClassKey = runtime.stillwindClassKey;
export const stillwindStyleKey = runtime.stillwindStyleKey;
export const stillwindMerge = runtime.stillwindMerge;
export const stillwindToFragment = runtime.stillwindToFragment;
