import type { APIRoute } from "astro";

export const prerender = true;

export const GET: APIRoute = ({ site }) =>
  new Response(
    `
User-agent: *
Allow: /

Sitemap: ${new URL("/sitemap-index.xml", site).href}
`.trimStart(),
    {
      headers: {
        "Content-Type": "text/plain; charset=utf-8",
      },
    },
  );
