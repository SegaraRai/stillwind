import { mergeFragmentsChecked, toFragmentChecked } from "./development.ts";
import { stillwindClassKey, stillwindStyleKey, stillwindCustomPropertiesKey } from "./solid.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

const options = {
  classProperty: "class",
  styleFormat: "object",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
} as const;

/** Checks fragment ownership and class channels before materializing solid props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting materialized solid props to fragment channels. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}
