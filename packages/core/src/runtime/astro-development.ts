import { options } from "./astro.ts";
import type { CompiledCvaConfig, CvaValue } from "./cva.ts";
import { createCvaChecked, mergeFragmentsChecked, toFragmentChecked } from "./development.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

/** Checks fragment ownership and class channels before materializing astro props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting a composition base to a astro class fragment. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}

/** Checks external classes and selected fragments before materializing astro CVA props. */
export function stillwindCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(options, base, config);
}
