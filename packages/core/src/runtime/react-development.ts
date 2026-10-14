import type { CompiledCvaConfig, CvaValue } from "./cva.ts";
import { createCvaChecked, mergeFragmentsChecked, toFragmentChecked } from "./development.ts";
import { options } from "./react.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

/** Checks fragment ownership and class channels before materializing react props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting a composition base to a react class fragment. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}

/** Checks external classes and selected fragments before materializing react CVA props. */
export function stillwindCvaChecked(base?: CvaValue, config?: CompiledCvaConfig) {
  return createCvaChecked(options, base, config);
}
