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
 *
 * Ref groups retain their first appearance order, and each marker precedes its
 * projections. Bases, utilities, and custom properties follow in that order. Each
 * category retains its input order.
 *
 * @param items - Inputs in their current order, with a distinct object for each occurrence.
 * @param allows - Optional predicate that approves the original range covering each adjacent
 * exchange. Rejected exchanges can prevent the result from reaching the full category order.
 * @returns A new array of the original objects. The input array and its objects remain unchanged.
 */
export function orderInputs<T extends InputOrderItem>(
  items: readonly T[],
  allows?: (start: number, end: number) => boolean,
): T[] {
  const sorted: T[] = [];
  let start = 0;
  for (let end = 0; end <= items.length; end++) {
    if (end === items.length || items[end].order.kind === "barrier") {
      sorted.push(...orderRun(items.slice(start, end), allows));
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
 * @param allows - Optional predicate over the original range covering two adjacent inputs.
 * @returns The same input objects in a new array. Neither the input array nor its objects change.
 */
function orderRun<T extends InputOrderItem>(
  items: readonly T[],
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
  const groups: Record<"bases" | "utilities" | "customProperties", T[]> = {
    bases: [],
    utilities: [],
    customProperties: [],
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
  const ordered = [
    ...[...refs.values()].flatMap(({ markers, projections }) => [...markers, ...projections]),
    ...groups.bases,
    ...groups.utilities,
    ...groups.customProperties,
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
