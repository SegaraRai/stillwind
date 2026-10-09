import type { CompiledCvaConfig, CvaValue } from "./cva.ts";
import { createCvaChecked, mergeFragmentsChecked, toFragmentChecked } from "./development.ts";
import { options } from "./vue.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

/** Checks fragment ownership and class channels before materializing vue props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting materialized vue props to fragment channels. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}

/** Checks external classes and selected fragments before materializing vue CVA props. */
export function stillwindCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(options, base, config);
}
