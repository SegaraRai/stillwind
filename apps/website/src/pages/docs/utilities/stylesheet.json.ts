import { recordDefaultStylesheet } from "../../../lib/docs/utilities/record";

export const prerender = true;

export async function GET() {
  return new Response(JSON.stringify(await recordDefaultStylesheet()), {
    headers: {
      "Content-Type": "application/json; charset=utf-8",
    },
  });
}
