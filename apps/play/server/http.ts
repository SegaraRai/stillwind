import type { IncomingMessage, ServerResponse } from "node:http";

const maximumPlayPayloadBytes = 1_000_000;

export function sendJson(response: ServerResponse, value: unknown, statusCode = 200) {
  response.statusCode = statusCode;
  response.setHeader("content-type", "application/json; charset=utf-8");
  response.setHeader("cache-control", "no-store");
  response.end(JSON.stringify(value));
}

export function readJson(request: IncomingMessage): Promise<{ source: string; theme: string }> {
  return new Promise((resolve, reject) => {
    let body = "";
    request.setEncoding("utf8");
    request.on("data", (chunk: string) => {
      body += chunk;
      if (Buffer.byteLength(body) > maximumPlayPayloadBytes) {
        reject(new Error("playground input exceeds 1 MB"));
      }
    });
    request.on("error", reject);
    request.on("end", () => {
      try {
        const value = JSON.parse(body) as Partial<{
          source: unknown;
          theme: unknown;
        }>;
        if (typeof value.source !== "string" || typeof value.theme !== "string") {
          throw new Error("source and theme must be strings");
        }
        resolve({ source: value.source, theme: value.theme });
      } catch (error) {
        reject(error);
      }
    });
  });
}
