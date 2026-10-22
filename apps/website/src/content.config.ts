import { docsCollection } from "@cloudflare/nimbus-docs/content";
import { z } from "astro/zod";
import { defineCollection } from "astro:content";

import { utilityCatalogLoader } from "./lib/utilities/catalog";

export const collections = {
  docs: defineCollection(docsCollection()),
  utilities: defineCollection({
    loader: utilityCatalogLoader(),
    schema: z.object({
      version: z.string(),
      revision: z.string(),
      records: z.array(z.object({ name: z.string(), css: z.string(), marker: z.boolean() })),
    }),
  }),
};
