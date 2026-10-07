import { expect, test } from "vite-plus/test";

import { orderInputs, type InputOrderItem, type InputOrder } from "./input-order.ts";

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
    { kind: "attributes", name: "data-z" },
    { kind: "utilities" },
    { kind: "attributes", name: "aria-label" },
  ]);
  const output = orderInputs(input);
  expect(output.map((item) => input.indexOf(item))).toEqual([6, 2, 5, 3, 1, 4, 0, 9, 7, 8, 10]);
  expect(orderInputs(output)).toEqual(output);
  expect(orderInputs(input, true).slice(-2)).toEqual([input[10], input[8]]);
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
  expect(orderInputs(restricted, false, (start, end) => start === 0 && end === 21)).toEqual(
    restricted,
  );
});

test("attribute objects delimit optional name sorting and duplicate names remain stable", () => {
  const input = items([
    { kind: "attributes", name: "data-z" },
    { kind: "attributes", name: "data-a" },
    { kind: "attributes", name: "data-a" },
    { kind: "attributes" },
    { kind: "attributes", name: "data-z" },
  ]);
  expect(orderInputs(input)).toEqual(input);
  expect(orderInputs(input, true)).toEqual([input[1], input[2], input[0], input[3], input[4]]);
});
