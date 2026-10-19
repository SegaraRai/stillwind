import { expect, test } from "vite-plus/test";

import { catalogInput } from "./plugins.ts";

test("catalog inputs keep the utility text of the candidate before the cursor", () => {
  expect(catalogInput('style("p-4 hover:icon-mdi--cl')).toBe("icon-mdi--cl");
  expect(catalogInput(".a { @apply [&:is(a,b)]:icon-[mdi--")).toBe("icon-[mdi--");
  expect(catalogInput("@apply flex ")).toBe("");
});
