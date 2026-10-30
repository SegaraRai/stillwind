import { afterEach, expect, test, vi } from "vite-plus/test";

import { checkSite, deploymentForBranch, needsInitialDeployment, previewUrl } from "./deploy.ts";

afterEach(() => vi.unstubAllGlobals());

test.each([
  { deployments: [], initialize: true },
  { deployments: [{ id: "existing-production-deployment" }], initialize: false },
])(
  "checks whether the Worker needs its first deployment: %j",
  async ({ deployments, initialize }) => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ success: true, result: { deployments } })),
    );
    await expect(needsInitialDeployment("stillwind-website", "account", "token")).resolves.toBe(
      initialize,
    );
  },
);

test("refuses to initialize when deployment history cannot be confirmed", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json({ success: false }, { status: 403 })),
  );
  await expect(needsInitialDeployment("stillwind-website", "account", "token")).rejects.toThrow(
    "HTTP 403",
  );
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json({ success: true, result: {} })),
  );
  await expect(needsInitialDeployment("stillwind-website", "account", "token")).rejects.toThrow(
    "no deployment history",
  );
});

test("initializes a missing Worker only when Cloudflare confirms that it does not exist", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      Response.json({ success: false, errors: [{ code: 10007 }] }, { status: 404 }),
    ),
  );
  await expect(needsInitialDeployment("stillwind-website", "account", "token")).resolves.toBe(true);
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => Response.json({ success: false }, { status: 404 })),
  );
  await expect(needsInitialDeployment("stillwind-website", "account", "token")).rejects.toThrow(
    "HTTP 404",
  );
});

test("routes only main to Preview and stable to production", () => {
  expect(deploymentForBranch("main")).toBe("preview");
  expect(deploymentForBranch("stable")).toBe("production");
  expect(() => deploymentForBranch("feature/test")).toThrow("main or stable");
});

test("selects the configured origin when cf returns multiple Preview domains", () => {
  expect(
    previewUrl(
      JSON.stringify(
        {
          type: "preview",
          preview_urls: [
            "https://main-stillwind-play.example.workers.dev",
            "https://main.play-preview.stillwind.dev",
          ],
          deployment_urls: ["https://123-stillwind-play.example.workers.dev"],
        },
        null,
        2,
      ),
      "https://main.play-preview.stillwind.dev/",
    ),
  ).toBe("https://main.play-preview.stillwind.dev/");
  expect(() =>
    previewUrl('{"type":"preview","preview_urls":[]}', "https://main.play-preview.stillwind.dev/"),
  ).toThrow("Preview URL");
  expect(() =>
    previewUrl(
      '{"type":"preview","preview_urls":["https://other.example/"]}',
      "https://main.play-preview.stillwind.dev/",
    ),
  ).toThrow("Preview URL");
  expect(() =>
    previewUrl(
      '{"type":"preview","preview_urls":["http://main.play-preview.stillwind.dev/"]}',
      "http://main.play-preview.stillwind.dev/",
    ),
  ).toThrow("HTTPS");
});

test("verifies Preview indexing, isolation, and the WASM MIME type", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async (url: URL) =>
        new Response(
          `<link rel="canonical" href="${url.origin}/">
          <meta property="og:url" content="${url.origin}/">
          <meta property="og:image" content="${url.origin}/og.png">
          <meta name="twitter:image" content="${url.origin}/og.png">`,
          {
            headers: {
              "Content-Type": "application/wasm",
              "Cross-Origin-Opener-Policy": "same-origin",
              "Cross-Origin-Embedder-Policy": "require-corp",
              "X-Robots-Tag": "noindex",
            },
          },
        ),
    ),
  );
  await expect(checkSite("play", "https://preview.example", "preview")).resolves.toBeUndefined();
  await expect(checkSite("play", "https://preview.example", "production")).rejects.toThrow(
    "indexing",
  );
});

test("rejects metadata that points to another deployment", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async () =>
        new Response('<link rel="canonical" href="https://other.example/">', {
          headers: {
            "Cross-Origin-Opener-Policy": "same-origin",
            "Cross-Origin-Embedder-Policy": "require-corp",
            "X-Robots-Tag": "noindex",
          },
        }),
    ),
  );
  await expect(checkSite("website", "https://preview.example", "preview")).rejects.toThrow(
    "canonical",
  );
});

test.each([
  { route: "/docs/", status: 307, location: "/docs", error: undefined },
  { route: "/docs/", status: 200, location: undefined, error: "redirect" },
  { route: "/docs/", status: 307, location: "/docs/", error: "redirect" },
  { route: "/docs", status: 307, location: "/docs/", error: "HTTP 307" },
])("checks slashless routes and redirects without following them: %j", async (result) => {
  const request = vi.fn(async (url: URL, init: RequestInit) => {
    if (url.pathname === result.route) {
      return new Response(null, {
        status: result.status,
        headers: result.location ? { Location: result.location } : {},
      });
    }
    if (url.pathname !== "/" && url.pathname.endsWith("/")) {
      return new Response(null, {
        status: 307,
        headers: { Location: url.pathname.slice(0, -1) },
      });
    }
    return new Response(
      init.method === "HEAD"
        ? null
        : `<link rel="canonical" href="${url.origin}/">
          <meta property="og:url" content="${url.origin}/">
          <meta property="og:image" content="${url.origin}/og.png">
          <meta name="twitter:image" content="${url.origin}/og.png">`,
      {
        headers: {
          "Cross-Origin-Opener-Policy": "same-origin",
          "Cross-Origin-Embedder-Policy": "require-corp",
        },
      },
    );
  });
  vi.stubGlobal("fetch", request);
  const check = checkSite("website", "https://stillwind.dev", "production");
  if (result.error) {
    await expect(check).rejects.toThrow(result.error);
  } else {
    await expect(check).resolves.toBeUndefined();
    for (const [, init] of request.mock.calls) {
      expect(init.redirect).toBe("manual");
    }
  }
});

test("rejects a successful HTML fallback for a missing WASM file", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async () =>
        new Response(null, {
          headers: {
            "Content-Type": "text/html",
            "Cross-Origin-Opener-Policy": "same-origin",
            "Cross-Origin-Embedder-Policy": "require-corp",
          },
        }),
    ),
  );
  await expect(checkSite("play", "https://play.example", "production")).rejects.toThrow(
    "application/wasm",
  );
});
