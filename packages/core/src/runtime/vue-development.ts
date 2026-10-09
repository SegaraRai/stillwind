import { mergeFragmentsChecked, toFragmentChecked } from "./development.ts";
import { stillwindClassKey, stillwindCustomPropertiesKey, stillwindStyleKey } from "./vue.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

const options = [
  "class",
  "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
] as const;

/** Checks fragment ownership and class channels before materializing vue props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting materialized vue props to fragment channels. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}
