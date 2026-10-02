/**
 * Inserts trusted stylesheet markup before the first closing head tag in a UTF-8 HTML response.
 * Preserves the response status and headers except content-length and etag, whose values describe the original body.
 * Returns a streaming response; documents without a closing head retain their original content.
 */
export function insertStyles(response: Response, styles: string): Response {
  if (!response.body) {
    return response;
  }

  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let pending = "";
  let inserted = false;
  const body = response.body.pipeThrough(
    new TransformStream<Uint8Array, Uint8Array>({
      transform(chunk, controller) {
        const text = decoder.decode(chunk, { stream: true });
        if (inserted) {
          controller.enqueue(encoder.encode(text));
          return;
        }
        pending += text;
        const index = pending.toLowerCase().indexOf("</head>");
        if (index < 0) {
          return;
        }
        controller.enqueue(encoder.encode(pending.slice(0, index) + styles + pending.slice(index)));
        pending = "";
        inserted = true;
      },
      flush(controller) {
        controller.enqueue(encoder.encode(pending + decoder.decode()));
      },
    }),
  );

  const headers = new Headers(response.headers);
  headers.delete("content-length");
  headers.delete("etag");

  return new Response(body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
