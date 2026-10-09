import type { CompiledCvaConfig, CvaValue } from "./cva.ts";
import { createCvaChecked, mergeFragmentsChecked, toFragmentChecked } from "./development.ts";
import { options } from "./svelte.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

/** Checks fragment ownership and class channels before materializing svelte props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting materialized svelte props to fragment channels. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}

/** Checks external classes and selected fragments before materializing svelte CVA props. */
export function stillwindCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(options, base, config);
}
