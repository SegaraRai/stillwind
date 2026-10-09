import { expect, test } from "vite-plus/test";

import { insertStyles } from "./document.ts";

test("styles are inserted before a closing head tag split across stream chunks", async () => {
  const html = "<html><head><title>日本語</title></HEAD><body>🧭</body></html>";
  const bytes = new TextEncoder().encode(html);
  const body = new ReadableStream<Uint8Array>({
    /**
     * Sends HTML one byte at a time to exercise split UTF-8 characters and head tags.
     * @param controller - Stream controller receiving the byte sequence and end-of-stream signal.
     */
    start(controller) {
      for (const byte of bytes) {
        controller.enqueue(new Uint8Array([byte]));
      }
      controller.close();
    },
  });
  const stylesheet = '<link rel="stylesheet" href="/style.css">';

  const response = insertStyles(new Response(body), stylesheet);

  expect(await response.text()).toBe(html.replace("</HEAD>", `${stylesheet}</HEAD>`));
});

test("inserting styles keeps the response status and cookies and drops stale body headers", () => {
  const html = "<html><head></head></html>";
  const originalResponse = new Response(html, {
    status: 201,
    headers: {
      "content-type": "text/html; charset=utf-8",
      "content-length": String(new TextEncoder().encode(html).length),
      etag: "original",
      "set-cookie": "session=abc",
    },
  });

  const response = insertStyles(originalResponse, "<style></style>");

  expect(response.status).toBe(201);
  expect(response.headers.get("content-type")).toBe("text/html; charset=utf-8");
  expect(response.headers.get("set-cookie")).toBe("session=abc");
  expect(response.headers.has("content-length")).toBe(false);
  expect(response.headers.has("etag")).toBe(false);
});

test("an HTML fragment without a head tag is returned unchanged", async () => {
  const fragment = "<p>fragment</p>";

  const response = insertStyles(new Response(fragment), "<style></style>");

  expect(await response.text()).toBe(fragment);
});
