import { expect, test } from "vite-plus/test";

import { resolveUtility, type DynamicUtility } from "./candidates.ts";

test("length registrations accept font-relative and viewport arbitrary values", () => {
  const registration: DynamicUtility = {
    owner: 0,
    name: "measure",
    callback: (value) => ({ width: value }),
    options: { type: "length" },
    markerSource: "",
    results: new Map(),
  };
  const units = [
    "px",
    "cap",
    "ic",
    "rex",
    "rch",
    "rcap",
    "ric",
    "svi",
    "svb",
    "svmin",
    "svmax",
    "lvi",
    "lvb",
    "lvmin",
    "lvmax",
    "dvi",
    "dvb",
    "dvmin",
    "dvmax",
  ];
  for (const unit of units) {
    const value = `10${unit}`;
    expect(resolveUtility(`measure-[${value}]`, registration), value).toEqual([`width:${value};`]);
  }
  for (const value of ["red", "10deg", "10furlong"]) {
    expect(resolveUtility(`measure-[${value}]`, registration), value).toBeUndefined();
  }
});
