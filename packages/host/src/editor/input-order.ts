/** Syntax categories used to group direct style inputs. */
export type InputOrder =
  | {
      /** Prevents inputs on either side from exchanging positions. */
      kind: "barrier";
    }
  | {
      /** Groups a ref marker with projections of the same declaration. */
      kind: "ref";
      /** Opaque declaration identity. Aliases of the same ref must share this value. */
      identity: string;
      /** True for a direct projection call, false for a marker attachment. */
      projection: boolean;
    }
  | {
      /** Identifies composed styles whose relative order must be retained. */
      kind: "bases";
    }
  | {
      /** Identifies utility text and named variant groups. */
      kind: "utilities";
    }
  | {
      /** Identifies authored custom-property inputs. */
      kind: "customProperties";
    }
  | {
      /** Identifies aria or data attributes, including containers of those attributes. */
      kind: "attributes";
      /** Decoded name of one attribute. Omission delimits name sorting for a container. */
      name?: string;
    };

/** Original source bounds and syntax category of one input. */
export interface InputOrderItem {
  /** Inclusive UTF-16 position in the original document. */
  start: number;
  /** Exclusive UTF-16 position in the original document. */
  end: number;
  /** A syntax category, or a barrier that no input can cross. */
  order: InputOrder;
}

/**
 * Groups `items` by syntax category without crossing barriers.
 * Ref groups retain their first appearance order, and each marker precedes its projections.
 * Bases, utilities, custom properties, and attributes follow in that order. Each
 * category retains its input order unless attribute name sorting is enabled.
 *
 * @param items - Inputs in their current order, with a distinct object for each occurrence.
 * @param attributes - Enables case-sensitive lexical ordering of named attributes after grouping.
 * Entries without a single name delimit that ordering, and equal names retain their order.
 * @param allows - Optional predicate that approves the original range covering each adjacent
 * exchange. Rejected exchanges can prevent the result from reaching the full category order.
 * @returns A new array of the original objects. The input array and its objects remain unchanged.
 */
export function orderInputs<T extends InputOrderItem>(
  items: readonly T[],
  attributes = false,
  allows?: (start: number, end: number) => boolean,
): T[] {
  const sorted: T[] = [];
  let start = 0;
  for (let end = 0; end <= items.length; end++) {
    if (end === items.length || items[end].order.kind === "barrier") {
      sorted.push(...orderRun(items.slice(start, end), attributes, allows));
      if (end < items.length) {
        sorted.push(items[end]);
      }
      start = end + 1;
    }
  }
  return sorted;
}

/**
 * Computes the category order for one run and applies any permitted adjacent exchanges.
 *
 * @param items - A run without barriers, with a distinct object for each occurrence.
 * @param attributes - Enables the attribute name ordering described by {@link orderInputs}.
 * @param allows - Optional predicate over the original range covering two adjacent inputs.
 * @returns The same input objects in a new array. Neither the input array nor its objects change.
 */
function orderRun<T extends InputOrderItem>(
  items: readonly T[],
  attributes: boolean,
  allows?: (start: number, end: number) => boolean,
): T[] {
  const refs = new Map<
    string,
    {
      /** Marker occurrences in their original relative order. */
      markers: T[];
      /** Projection occurrences in their original relative order. */
      projections: T[];
    }
  >();
  const groups: Record<"bases" | "utilities" | "customProperties" | "attributes", T[]> = {
    bases: [],
    utilities: [],
    customProperties: [],
    attributes: [],
  };
  for (const item of items) {
    if (item.order.kind === "ref") {
      let group = refs.get(item.order.identity);
      if (!group) {
        group = { markers: [], projections: [] };
        refs.set(item.order.identity, group);
      }
      group[item.order.projection ? "projections" : "markers"].push(item);
    } else if (item.order.kind !== "barrier") {
      groups[item.order.kind].push(item);
    }
  }
  if (attributes) {
    // Entries without a single name delimit attribute objects or nested arrays.
    let start = 0;
    for (let end = 0; end <= groups.attributes.length; end++) {
      if (end === groups.attributes.length || attributeName(groups.attributes[end]) === undefined) {
        groups.attributes.splice(
          start,
          end - start,
          ...groups.attributes.slice(start, end).sort((a, b) => {
            const left = attributeName(a)!;
            const right = attributeName(b)!;
            return left < right ? -1 : left > right ? 1 : 0;
          }),
        );
        start = end + 1;
      }
    }
  }
  const ordered = [
    ...[...refs.values()].flatMap(({ markers, projections }) => [...markers, ...projections]),
    ...groups.bases,
    ...groups.utilities,
    ...groups.customProperties,
    ...groups.attributes,
  ];
  if (!allows) {
    return ordered;
  }
  const positions = new Map(ordered.map((item, index) => [item, index]));
  const sorted = [...items];
  for (let position = 1; position < sorted.length; position++) {
    for (let cursor = position; cursor > 0; cursor--) {
      const left = sorted[cursor - 1];
      const right = sorted[cursor];
      if (
        positions.get(left)! <= positions.get(right)! ||
        !allows(Math.min(left.start, right.start), Math.max(left.end, right.end))
      ) {
        break;
      }
      [sorted[cursor - 1], sorted[cursor]] = [right, left];
    }
  }
  return sorted;
}

/**
 * Supplies the name used to sort individual attributes within a run.
 *
 * @param item - An input classified by its authored syntax.
 * @returns The decoded attribute name, including an empty name when present, or undefined
 * for a container or another input category. Undefined delimits name sorting.
 */
function attributeName(item: InputOrderItem): string | undefined {
  return item.order.kind === "attributes" ? item.order.name : undefined;
}
