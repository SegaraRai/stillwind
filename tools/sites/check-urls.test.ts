import { expect, test } from "vite-plus/test";

import { checkPageUrls, siteUrls } from "./check-urls.ts";

function page(url: string, image: string): string {
  return `<link rel="canonical" href="${url}">
    <meta property="og:url" content="${url}">
    <meta property="og:image" content="${image}">
    <meta name="twitter:image" content="${image}">`;
}

test.each([
  ["https://main-website.example.workers.dev/", "https://main-play.example.workers.dev/"],
  ["https://stillwind.dev/", "https://play.stillwind.dev/"],
])("metadata uses the configured website and playground origins (%s)", (website, play) => {
  const urls = siteUrls({
    PUBLIC_WEBSITE_URL: website,
    PUBLIC_PLAYGROUND_URL: play,
  });
  expect(urls).toEqual({
    website,
    play,
  });
  for (const url of [website, new URL("docs/", website).href, play]) {
    const image = new URL("/og.png", url).href;
    expect(() => checkPageUrls(page(url, image), url, image)).not.toThrow();
    expect(() => checkPageUrls(page(url, "https://other.example/og.png"), url, image)).toThrow(
      "og:image",
    );
  }
});

test("builds default to the production origins", () => {
  expect(siteUrls({})).toEqual({
    website: "https://stillwind.dev/",
    play: "https://play.stillwind.dev/",
  });
  expect(() =>
    siteUrls({
      PUBLIC_WEBSITE_URL: "https://example.com/subpath/",
    }),
  ).toThrow("must be an HTTP(S) origin");
});
