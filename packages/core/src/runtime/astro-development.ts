import { stillwindClassKey, stillwindStyleKey, stillwindCustomPropertiesKey } from "./astro.ts";
import { mergeFragmentsChecked, toFragmentChecked } from "./development.ts";

export {
  stillwindClsxChecked,
  stillwindRefClassChecked,
  stillwindViewTransitionNameChecked,
} from "./development.ts";

const options = {
  classProperty: "class",
  styleFormat: "string",
  stillwindClassKey,
  stillwindStyleKey,
  stillwindCustomPropertiesKey,
} as const;

/** Checks fragment ownership and class channels before materializing astro props. */
export function stillwindMergeChecked(...values: readonly unknown[]): Record<string, unknown> {
  return mergeFragmentsChecked(options, values);
}

/** Rejects nonobjects before converting materialized astro props to fragment channels. */
export function stillwindToFragmentChecked(value: unknown): Record<PropertyKey, unknown> {
  return toFragmentChecked(options, value);
}
