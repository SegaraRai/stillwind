import { expect, test } from "vite-plus/test";

import { breadcrumbList, serializeStructuredData } from "./structured-data";

test("numbers linked breadcrumbs with absolute URLs and skips groups without a page", () => {
  const list = breadcrumbList(
    [
      {
        label: "Home",
        href: "/",
      },
      {
        label: "Writing styles",
      },
      {
        label: "Animations",
        href: "/docs/guides/animations",
      },
    ],
    new URL("https://example.com"),
  );

  expect(list.itemListElement).toEqual([
    {
      "@type": "ListItem",
      position: 1,
      name: "Home",
      item: "https://example.com/",
    },
    {
      "@type": "ListItem",
      position: 2,
      name: "Animations",
      item: "https://example.com/docs/guides/animations",
    },
  ]);
});

test("keeps a string value from closing the script element", () => {
  const value = "</script>";
  const serialized = serializeStructuredData({ value });

  expect(serialized).not.toContain("<");
  expect(JSON.parse(serialized)).toEqual({ value });
});
