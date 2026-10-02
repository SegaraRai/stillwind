/// <reference types="astro/client" />

import { defineMiddleware } from "astro:middleware";
import { getStyles } from "virtual:stillwind/astro-styles";

import { insertStyles } from "./document.ts";

export const onRequest = defineMiddleware(async (context, next) => {
  const response = await next();
  if (!response.body || !response.headers.get("content-type")?.startsWith("text/html")) {
    return response;
  }
  const styles = await getStyles(context.request.url);
  if (!styles) {
    return response;
  }
  return insertStyles(response, styles);
});
