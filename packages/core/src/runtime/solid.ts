import { createFragmentRuntime } from "./fragment.ts";

export * from "./shared.ts";

const runtime = createFragmentRuntime({
  classProperty: "class",
  styleFormat: "object",
});

export const stillwindClassKey = runtime.stillwindClassKey;
export const stillwindCustomPropertiesKey = runtime.stillwindCustomPropertiesKey;
export const stillwindStyleKey = runtime.stillwindStyleKey;
export const stillwindMerge = runtime.stillwindMerge;
export const stillwindToFragment = runtime.stillwindToFragment;
export const stillwindStyleValue = runtime.stillwindStyleValue;
