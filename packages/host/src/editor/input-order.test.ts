import { expect, test } from "vite-plus/test";

import { orderInputs, type InputOrder, type InputOrderItem } from "./input-order.ts";

/**
 * Gives each input an independent range so tests can identify permitted exchanges.
 *
 * @param orders - Categories in the sequence that the test supplies to the sorter.
 * @returns New items with one-unit ranges spaced ten units apart, in the supplied order.
 */
function items(orders: InputOrder[]): InputOrderItem[] {
  return orders.map((order, index) => ({ start: index * 10, end: index * 10 + 1, order }));
}

test("groups retain original order and each ref keeps its marker before its projections", () => {
  const input = items([
    { kind: "utilities" },
    { kind: "bases" },
    { kind: "ref", identity: "b", projection: true },
    { kind: "ref", identity: "a", projection: true },
    { kind: "bases" },
    { kind: "ref", identity: "a", projection: false },
    { kind: "ref", identity: "b", projection: false },
    { kind: "customProperties" },
    { kind: "utilities" },
  ]);
  const output = orderInputs(input);
  expect(output.map((item) => input.indexOf(item))).toEqual([6, 2, 5, 3, 1, 4, 0, 8, 7]);
  expect(orderInputs(output)).toEqual(output);
});

test("barriers delimit runs and arbitrary range predicates can reject individual exchanges", () => {
  const input = items([
    { kind: "utilities" },
    { kind: "bases" },
    { kind: "barrier" },
    { kind: "bases" },
    { kind: "ref", identity: "a", projection: false },
  ]);
  expect(orderInputs(input)).toEqual([input[1], input[0], input[2], input[4], input[3]]);
  const restricted = items([
    { kind: "utilities" },
    { kind: "bases" },
    { kind: "ref", identity: "a", projection: false },
  ]);
  expect(orderInputs(restricted, (start, end) => start === 0 && end === 21)).toEqual(restricted);
});
