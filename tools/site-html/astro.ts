import { readdir, readFile, writeFile } from "node:fs/promises";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

import type { AstroIntegration } from "astro";

import {
  contentSecurityPolicyRule,
  inlineScriptSources,
  scriptSource,
  withCloudflareWebAnalytics,
  type ContentSecurityPolicy,
} from "./content-security-policy.ts";
import { minifyInlineScripts } from "./minify.ts";

/**
 * Options for {@link siteHtml}.
 */
export interface SiteHtmlOptions {
  /** Policy for every response. The integration adds the hashes of inline scripts to `script-src`. */
  contentSecurityPolicy: ContentSecurityPolicy;
  /** Whether the policy also allows the beacon and reports of Cloudflare Web Analytics. */
  cloudflareWebAnalytics?: boolean;
  /**
   * Returns the text of inline scripts that pages create at runtime, such as the script in a
   * `srcdoc` frame. `output` is the build output directory. Each returned text must match the
   * text of the created element exactly.
   */
  runtimeInlineScripts?: (output: URL) => Promise<readonly string[]>;
}

/**
 * Creates an Astro integration for a static site deployed with a Cloudflare `_headers` file.
 * After a build, it minifies the inline scripts of every HTML page that Astro rendered. Other HTML
 * files in the output stay unchanged, because a copied file, such as a library's frame document,
 * can allow its own script by hash. Then it appends a rule to `_headers` that sends the Content
 * Security Policy with every response. The rule allows each inline script of every HTML file in
 * the output by its hash. Development servers send no policy and serve the scripts unchanged.
 * The build fails when a script cannot be minified or when the policy cannot be served.
 */
export function siteHtml(options: SiteHtmlOptions): AstroIntegration {
  return {
    name: "stillwind-internal:site-html",
    hooks: {
      "astro:build:done": async ({ dir, assets, logger }) => {
        const root = fileURLToPath(dir);
        const pages = new Set(
          [...assets.values()]
            .flat()
            .map((url) => fileURLToPath(url))
            .filter((path) => path.endsWith(".html")),
        );
        const files = (await readdir(root, { recursive: true }))
          .filter((file) => file.endsWith(".html"))
          .map((file) => join(root, file));
        const sources = new Set<string>();
        await Promise.all(
          files.map(async (path) => {
            let html = await readFile(path, "utf8");
            if (pages.has(path)) {
              const minified = await minifyInlineScripts(html, relative(root, path));
              if (minified !== html) {
                await writeFile(path, minified);
                html = minified;
              }
            }
            inlineScriptSources(html).forEach((source) => sources.add(source));
          }),
        );
        for (const code of (await options.runtimeInlineScripts?.(dir)) ?? []) {
          sources.add(scriptSource(code));
        }

        const headers = join(root, "_headers");
        let existing = "";
        try {
          existing = await readFile(headers, "utf8");
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
            throw error;
          }
        }
        const policy = options.cloudflareWebAnalytics
          ? withCloudflareWebAnalytics(options.contentSecurityPolicy)
          : options.contentSecurityPolicy;
        const rule = contentSecurityPolicyRule(policy, sources);
        await writeFile(headers, existing ? `${existing.trimEnd()}\n\n${rule}` : rule);
        logger.info(
          `Minified inline scripts in ${pages.size} HTML pages. The Content Security Policy allows ${sources.size} inline scripts.`,
        );
      },
    },
  };
}
