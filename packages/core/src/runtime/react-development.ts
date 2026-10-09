import { mergeFragmentsChecked, toFragmentChecked } from "./development.ts";
import { stillwindClassKey, stillwindCustomPropertiesKey, stillwindStyleKey } from "./react.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

const options = [
  "className",
  "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
] as const;

/** Checks fragment ownership and class channels before materializing react props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting materialized react props to fragment channels. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}
