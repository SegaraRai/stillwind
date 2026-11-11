import { createHash } from "node:crypto";

import { findScripts } from "./scripts.ts";

/**
 * Reports a Content Security Policy that the site cannot serve.
 */
export class ContentSecurityPolicyError extends Error {
  override name = "ContentSecurityPolicyError";
}

/**
 * Directives of a Content Security Policy. Each key is a directive name, such as `script-src`,
 * and each value lists its sources in serialized form, such as `'self'`.
 */
export type ContentSecurityPolicy = Readonly<Record<string, readonly string[]>>;

/**
 * Cloudflare rejects a `_headers` line longer than this number of characters.
 */
const headerLineLimit = 2000;

/**
 * Returns a `_headers` rule that sends the policy with every response of the site.
 * `scriptSources` are added to `script-src` once each, in sorted order.
 * Throws a {@link ContentSecurityPolicyError} when the policy has no `script-src` directive or when
 * the header line exceeds the Cloudflare limit.
 */
export function contentSecurityPolicyRule(
  policy: ContentSecurityPolicy,
  scriptSources: Iterable<string>,
): string {
  if (!policy["script-src"]) {
    throw new ContentSecurityPolicyError("The Content Security Policy must set script-src.");
  }
  const hashes = [...new Set(scriptSources)].sort();
  const value = Object.entries(policy)
    .map(([directive, sources]) =>
      [directive, ...sources, ...(directive === "script-src" ? hashes : [])].join(" "),
    )
    .join("; ");
  const line = `  Content-Security-Policy: ${value}`;
  if (line.length > headerLineLimit) {
    throw new ContentSecurityPolicyError(
      `The Content-Security-Policy line has ${line.length} characters. Cloudflare accepts at most ${headerLineLimit}.`,
    );
  }
  return `/*\n${line}\n`;
}

/**
 * Returns the hash sources of the inline scripts that the browser runs in an HTML document.
 * External scripts and data blocks have no hash source.
 */
export function inlineScriptSources(html: string): string[] {
  return findScripts(html)
    .filter((script) => script.inline)
    .map((script) => scriptSource(script.code));
}

/**
 * Returns the `'sha256-…'` source that allows an inline script with the given text.
 * The text must be the raw element text from an HTML document or a `srcdoc` value. The hash uses
 * line feeds for CR LF and lone CR, because the HTML parser normalizes them before CSP checks.
 */
export function scriptSource(code: string): string {
  const text = code.replace(/\r\n?/g, "\n");
  return `'sha256-${createHash("sha256").update(text).digest("base64")}'`;
}

/**
 * Sources that Cloudflare Web Analytics loads its beacon script from and sends reports to.
 */
const cloudflareWebAnalytics: ContentSecurityPolicy = {
  "script-src": ["https://static.cloudflareinsights.com"],
  "connect-src": ["https://cloudflareinsights.com"],
};

/**
 * Returns a copy of the policy that also allows Cloudflare Web Analytics.
 * A directive that the policy does not set starts from the `default-src` sources, so the sources
 * that it inherited stay allowed.
 */
export function withCloudflareWebAnalytics(policy: ContentSecurityPolicy): ContentSecurityPolicy {
  const result: Record<string, readonly string[]> = { ...policy };
  for (const [directive, sources] of Object.entries(cloudflareWebAnalytics)) {
    const current = policy[directive] ?? policy["default-src"] ?? [];
    result[directive] = [...current, ...sources.filter((source) => !current.includes(source))];
  }
  return result;
}
