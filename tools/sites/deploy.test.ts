import { afterEach, expect, test, vi } from "vite-plus/test";

import { checkSite, deploymentForBranch, previewUrl } from "./deploy.ts";

afterEach(() => vi.unstubAllGlobals());

test("routes only main to Preview and stable to production", () => {
  expect(deploymentForBranch("main")).toBe("preview");
  expect(deploymentForBranch("stable")).toBe("production");
  expect(() => deploymentForBranch("feature/test")).toThrow("main or stable");
});

test("reads the branch Preview URL from cf output rather than a temporary deployment URL", () => {
  expect(
    previewUrl(
      JSON.stringify(
        {
          type: "preview",
          preview_urls: ["https://main-stillwind-play.example.workers.dev"],
          deployment_urls: ["https://123-stillwind-play.example.workers.dev"],
        },
        null,
        2,
      ),
    ),
  ).toBe("https://main-stillwind-play.example.workers.dev/");
  expect(() => previewUrl('{"type":"preview","preview_urls":[]}')).toThrow("Preview URL");
});

test("verifies Preview indexing, isolation, and the WASM MIME type", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async () =>
        new Response(null, {
          headers: {
            "Content-Type": "application/wasm",
            "Cross-Origin-Opener-Policy": "same-origin",
            "Cross-Origin-Embedder-Policy": "require-corp",
            "X-Robots-Tag": "noindex",
          },
        }),
    ),
  );
  await expect(checkSite("play", "https://preview.example", "preview")).resolves.toBeUndefined();
  await expect(checkSite("play", "https://preview.example", "production")).rejects.toThrow(
    "indexing",
  );
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
