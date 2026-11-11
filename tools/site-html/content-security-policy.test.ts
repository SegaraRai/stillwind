import { expect, test } from "vite-plus/test";

import {
  ContentSecurityPolicyError,
  contentSecurityPolicyRule,
  inlineScriptSources,
  scriptSource,
  withCloudflareWebAnalytics,
} from "./content-security-policy.ts";

test("hashes script text after the HTML parser normalizes line breaks", () => {
  // The CSP specification uses this script and hash as its example.
  expect(scriptSource("alert('Hello, world.');")).toBe(
    "'sha256-qznLcsROx4GACP2dm0UCKCzCG+HiZ1guq6ZZDob/Tng='",
  );
  expect(scriptSource("a();\r\nb();\rc();")).toBe(scriptSource("a();\nb();\nc();"));
});

test("returns hash sources only for inline scripts that the browser runs", () => {
  const html = [
    `<script>a();</script>`,
    `<script type="module">b();</script>`,
    `<script type="module" src="/_astro/page.js"></script>`,
    `<script type="application/ld+json">{}</script>`,
  ].join("");

  expect(inlineScriptSources(html)).toEqual([scriptSource("a();"), scriptSource("b();")]);
});

test("adds each script hash once to script-src in a rule for every path", () => {
  const rule = contentSecurityPolicyRule(
    {
      "default-src": ["'self'"],
      "script-src": ["'self'"],
      "object-src": ["'none'"],
    },
    ["'sha256-b'", "'sha256-a'", "'sha256-b'"],
  );

  expect(rule).toBe(
    "/*\n  Content-Security-Policy: default-src 'self'; script-src 'self' 'sha256-a' 'sha256-b'; object-src 'none'\n",
  );
});

test("rejects a policy without script-src", () => {
  expect(() =>
    contentSecurityPolicyRule(
      {
        "default-src": ["'self'"],
      },
      [],
    ),
  ).toThrow(ContentSecurityPolicyError);
});

test("allows Cloudflare Web Analytics and keeps the sources that a new directive inherited", () => {
  expect(
    withCloudflareWebAnalytics({
      "default-src": ["'self'"],
      "script-src": ["'self'"],
    }),
  ).toEqual({
    "default-src": ["'self'"],
    "script-src": ["'self'", "https://static.cloudflareinsights.com"],
    "connect-src": ["'self'", "https://cloudflareinsights.com"],
  });
});
