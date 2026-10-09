/**
 * Inserts trusted stylesheet markup before the first closing head tag in a UTF-8 HTML response.
 * Preserves the response status and headers except content-length and etag, whose values describe the original body.
 * Returns a streaming response; documents without a closing head retain their original content.
 * @param response - Response whose body contains UTF-8 HTML. A missing body returns the same response.
 * @param styles - Trusted markup inserted without escaping before the first case-insensitive closing head tag.
 * @returns A response that consumes the original body stream and removes content-length and etag.
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
      /**
       * Decodes a chunk and buffers text until the closing head tag permits insertion.
       * @param chunk - Next bytes from the original UTF-8 stream.
       * @param controller - Receives encoded output after insertion or for subsequent chunks.
       */
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
      /**
       * Emits remaining text and completes UTF-8 decoding when the input stream ends.
       * @param controller - Receives buffered HTML when no closing head was found, or trailing decoder text.
       */
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
