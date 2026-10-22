import { getEntry } from "astro:content";

export const prerender = true;
export async function GET() {
  const entry = await getEntry("utilities", "default");
  if (!entry) {
    throw new Error("Default utility catalog was not loaded");
  }
  return new Response(JSON.stringify(entry.data), {
    headers: { "Content-Type": "application/json; charset=utf-8" },
  });
}
