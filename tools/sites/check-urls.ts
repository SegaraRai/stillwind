import { readFile, readdir } from "node:fs/promises";
import path from "node:path";

import { sites, type Site } from "./prepare.ts";

type SiteUrls = Record<Site, string>;

/**
 * Returns the two build origins. Rejects credentials, paths, queries, and fragments.
 */
export function siteUrls(environment: NodeJS.ProcessEnv): SiteUrls {
  const values = {
    website: environment.PUBLIC_WEBSITE_URL ?? sites.website.origin,
    play: environment.PUBLIC_PLAYGROUND_URL ?? sites.play.origin,
  };
  for (const site of ["website", "play"] as const) {
    const url = new URL(values[site]);
    if (
      !["http:", "https:"].includes(url.protocol) ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) {
      throw new Error(`${site}: the build URL must be an HTTP(S) origin`);
    }
    values[site] = url.href;
  }
  return values;
}

/**
 * Checks the canonical URL and social card URLs of a rendered page. Throws on missing or wrong URLs.
 */
export function checkPageUrls(html: string, pageUrl: string, imageUrl: string): void {
  const values = new Map<string, string>();
  for (const [tag] of html.matchAll(/<(?:meta|link)\b[^>]*>/g)) {
    const attributes = new Map(
      [...tag.matchAll(/\s([\w:-]+)="([^"]*)"/g)].map(([, name, value]) => [name!, value!]),
    );
    if (attributes.get("rel") === "canonical") {
      values.set("canonical", attributes.get("href") ?? "");
    } else {
      const name = attributes.get("property") ?? attributes.get("name");
      if (name) {
        values.set(name, attributes.get("content") ?? "");
      }
    }
  }
  for (const [name, expected] of [
    ["canonical", pageUrl],
    ["og:url", pageUrl],
    ["og:image", imageUrl],
    ["twitter:image", imageUrl],
  ] as const) {
    if (values.get(name) !== expected) {
      throw new Error(`${pageUrl}: ${name} must be ${expected}, received ${values.get(name)}`);
    }
  }
}

/**
 * Checks rendered metadata, navigation, sitemaps, and agent indexes against the build origins.
 */
export async function checkBuiltUrls(root: string, urls: SiteUrls): Promise<void> {
  const website = path.resolve(root, sites.website.source);
  const play = path.resolve(root, sites.play.source);
  const websiteOrigin = new URL(urls.website).origin;
  const files = await readdir(website, { recursive: true });
  for (const file of files) {
    const route = file.split(path.sep).join("/");
    if (route === "index.html" || (route.startsWith("docs/") && route.endsWith("/index.html"))) {
      const pageUrl = new URL(route.replace(/index\.html$/, ""), urls.website).href;
      checkPageUrls(
        await readFile(path.join(website, file), "utf8"),
        pageUrl,
        new URL("og.png", urls.website).href,
      );
    }
  }
  checkPageUrls(
    await readFile(path.join(play, "index.html"), "utf8"),
    urls.play,
    new URL("og.png", urls.play).href,
  );
  for (const file of ["index.html", "404.html"]) {
    const html = await readFile(path.join(website, file), "utf8");
    if (!html.includes(`href="${urls.play}"`)) {
      throw new Error(`website/${file}: missing link to ${urls.play}`);
    }
  }
  const docsUrl = new URL("docs/", urls.website).href;
  if (!(await readFile(path.join(play, "404.html"), "utf8")).includes(`href="${docsUrl}"`)) {
    throw new Error(`play/404.html: missing link to ${docsUrl}`);
  }
  for (const file of (await readdir(website)).filter((file) => /^sitemap.*\.xml$/.test(file))) {
    const xml = await readFile(path.join(website, file), "utf8");
    for (const [, location] of xml.matchAll(/<loc>([^<]+)<\/loc>/g)) {
      if (new URL(location!).origin !== websiteOrigin) {
        throw new Error(`${file}: sitemap URL belongs to another origin: ${location}`);
      }
    }
  }
  for (const [file, expected] of [
    ["sitemap-index.xml", new URL("sitemap-0.xml", urls.website).href],
    ["llms.txt", new URL("llms-full.txt", urls.website).href],
    ["llms-full.txt", docsUrl],
    ["docs/index.md", new URL("llms.txt", urls.website).href],
  ] as const) {
    if (!(await readFile(path.join(website, file), "utf8")).includes(expected)) {
      throw new Error(`${file}: missing URL ${expected}`);
    }
  }
  for (const file of files.filter((file) => path.basename(file) === "llms.txt")) {
    const markdown = await readFile(path.join(website, file), "utf8");
    for (const [, target] of markdown.matchAll(/\]\((https?:\/\/[^)]+)\)/g)) {
      if (new URL(target!).origin !== websiteOrigin) {
        throw new Error(`${file}: index link belongs to another origin: ${target}`);
      }
    }
  }
  for (const file of [".well-known/ai-catalog.json", ".well-known/ard.json"]) {
    const catalog = JSON.parse(await readFile(path.join(website, file), "utf8")) as {
      host: {
        identifier: string;
        documentationUrl: string;
      };
      entries: {
        url: string;
      }[];
    };
    if (
      catalog.host.identifier !== urls.website ||
      catalog.host.documentationUrl !== urls.website
    ) {
      throw new Error(`${file}: catalog host must be ${urls.website}`);
    }
    for (const entry of catalog.entries) {
      if (new URL(entry.url).origin !== websiteOrigin) {
        throw new Error(`${file}: catalog entry belongs to another origin: ${entry.url}`);
      }
    }
  }
  console.log(`Verified build URLs: ${urls.website} and ${urls.play}`);
}

if (import.meta.main) {
  await checkBuiltUrls(path.resolve(import.meta.dirname, "../.."), siteUrls(process.env));
}
