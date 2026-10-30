import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { expect, onTestFinished, test } from "vite-plus/test";

import { checkBuiltUrls, checkPageUrls, siteUrls } from "./check-urls.ts";
import { sites } from "./prepare.ts";

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
  for (const url of [website, new URL("docs", website).href, play]) {
    const image = new URL("/og.png", url).href;
    expect(() => checkPageUrls(page(url, image), url, image)).not.toThrow();
    expect(() => checkPageUrls(page(url, "https://other.example/og.png"), url, image)).toThrow(
      "og:image",
    );
  }
});

test("navigation uses slashless document URLs and preserves root and file links", () => {
  const url = "https://stillwind.dev/docs";
  const image = "https://stillwind.dev/og.png";
  const html = page(url, image);
  expect(() =>
    checkPageUrls(
      `${html}<a href="/">Home</a><a href="/docs/index.md">Markdown</a>
        <a href="/docs/guides/style#conditions">Style</a>`,
      url,
      image,
    ),
  ).not.toThrow();
  expect(() =>
    checkPageUrls(`${html}<a href="/docs/guides/style/#conditions">Style</a>`, url, image),
  ).toThrow("trailing slash");
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

test.each(["website", "play"] as const)(
  "%s URL checks need only that site's output",
  async (site) => {
    const root = await mkdtemp(path.join(tmpdir(), "stillwind-urls-"));
    onTestFinished(() => rm(root, { recursive: true, force: true }));
    const urls = siteUrls({
      PUBLIC_WEBSITE_URL: "https://docs.example/",
      PUBLIC_PLAYGROUND_URL: "https://play.example/",
    });
    const output = path.join(root, sites[site].source);
    const link = site === "website" ? urls.play : new URL("docs", urls.website).href;
    const files: Record<string, string> = {
      "index.html":
        page(urls[site], new URL("og.png", urls[site]).href) + `<a href="${link}">Open</a>`,
      "404.html": `<a href="${link}">Open</a>`,
    };
    if (site === "website") {
      Object.assign(files, {
        "docs/index.html": page(`${urls.website}docs`, `${urls.website}og.png`),
        "sitemap-index.xml": `<loc>${urls.website}sitemap-0.xml</loc>`,
        "sitemap-0.xml": `<loc>${urls.website}docs</loc>`,
        "llms.txt": `[Full](${urls.website}llms-full.txt)`,
        "llms-full.txt": `${urls.website}docs`,
        "docs/index.md": `${urls.website}llms.txt`,
        ...Object.fromEntries(
          ["ai-catalog", "ard"].map((name) => [
            `.well-known/${name}.json`,
            JSON.stringify({
              host: { identifier: urls.website, documentationUrl: urls.website },
              entries: [{ url: `${urls.website}docs` }],
            }),
          ]),
        ),
      });
    }
    for (const [file, contents] of Object.entries(files)) {
      await mkdir(path.dirname(path.join(output, file)), { recursive: true });
      await writeFile(path.join(output, file), contents);
    }
    await expect(checkBuiltUrls(root, urls, [site])).resolves.toBeUndefined();
    await writeFile(path.join(output, "404.html"), '<a href="https://wrong.example/">Open</a>');
    await expect(checkBuiltUrls(root, urls, [site])).rejects.toThrow("missing link");
  },
);
