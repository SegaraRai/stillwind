import { expect, test } from "vite-plus/test";

import { containerUtility } from "./container.ts";

const breakpoints = {
  md: "48rem",
  sm: "40rem",
  lg: "64rem",
};

test("center and padding add declarations, and padding keys follow the breakpoints", () => {
  expect(
    containerUtility(
      {
        center: true,
        padding: "2rem",
      },
      breakpoints,
    ),
  ).toBe("margin-inline:auto;padding-inline:2rem;");
  expect(
    containerUtility(
      {
        padding: {
          DEFAULT: "1rem",
          lg: "3rem",
          sm: "2rem",
        },
      },
      breakpoints,
    ),
  ).toBe(
    "padding-inline:1rem;@media (width >= 40rem){padding-inline:2rem;}@media (width >= 64rem){padding-inline:3rem;}",
  );
});

test("screens replace the maximum widths and carry the padding of the same name", () => {
  expect(
    containerUtility(
      {
        padding: {
          DEFAULT: "1rem",
          sm: "2rem",
          lg: "3rem",
        },
        screens: {
          sm: "600px",
          md: { min: "900px" },
          xl: { max: "1px" },
        },
      },
      breakpoints,
    ),
  ).toBe(
    "padding-inline:1rem;@media (width >= 40rem){max-width:none;}@media (width >= 600px){max-width:600px;padding-inline:2rem;}@media (width >= 900px){max-width:900px;}",
  );
  expect(containerUtility({}, breakpoints)).toBe("");
  expect(containerUtility(undefined, breakpoints)).toBe("");
});
