import type { ServerResponse } from "node:http";

/**
 * Ends a response with JSON and disables caching for development snapshots.
 * @param response - Writable response whose status and content-type/cache-control headers are replaced.
 * @param value - Value accepted by JSON.stringify.
 * @param statusCode - HTTP status to send, defaulting to 200.
 * @throws If serialization or writing the response fails.
 */
export function sendJson(response: ServerResponse, value: unknown, statusCode = 200) {
  response.statusCode = statusCode;
  response.setHeader("content-type", "application/json; charset=utf-8");
  response.setHeader("cache-control", "no-store");
  response.end(JSON.stringify(value));
}
