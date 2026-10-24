import type { GatsbyConfig } from "gatsby";

export default {
  flags: {
    DEV_SSR: true,
  },
  siteMetadata: {
    title: "Stillwind acceptance",
  },
  graphqlTypegen: false,
} satisfies GatsbyConfig;
