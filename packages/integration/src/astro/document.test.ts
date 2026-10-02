import { expect, test } from "vite-plus/test";

import { insertStyles } from "./document.ts";

test("inserts SSR styles across byte and head-tag boundaries and preserves response metadata", async () => {
  const html = "<html><head><title>日本語</title></HEAD><body>🧭</body></html>";
  const bytes = new TextEncoder().encode(html);
  const body = new ReadableStream<Uint8Array>({
    start(controller) {
      for (const byte of bytes) {
        controller.enqueue(new Uint8Array([byte]));
      }
      controller.close();
    },
  });
  const response = insertStyles(
    new Response(body, {
      status: 201,
      headers: {
        "content-type": "text/html; charset=utf-8",
        "content-length": String(bytes.length),
        etag: "original",
        "set-cookie": "session=abc",
      },
    }),
    '<link rel="stylesheet" href="/style.css">',
  );
  expect(await response.text()).toBe(
    html.replace("</HEAD>", '<link rel="stylesheet" href="/style.css"></HEAD>'),
  );
  expect(response.status).toBe(201);
  expect(response.headers.get("set-cookie")).toBe("session=abc");
  expect(response.headers.has("content-length")).toBe(false);
  expect(response.headers.has("etag")).toBe(false);
});

test("preserves fragments that contain no document head", async () => {
  expect(await insertStyles(new Response("<p>fragment</p>"), "<style></style>").text()).toBe(
    "<p>fragment</p>",
  );
});
