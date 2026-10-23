import { options } from "./class-string.ts";
import type { CompiledCvaConfig, CvaValue } from "./cva.ts";
import { createCvaChecked, mergeFragmentsChecked, toFragmentChecked } from "./development.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

/** Checks fragment ownership and class contributions before materializing `class` and string styles. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before extracting the composition base's `class` into a fragment. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}

/** Checks external classes and selected fragments before materializing CVA props with `class` and string styles. */
export function stillwindCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(options, base, config);
}
