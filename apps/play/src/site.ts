export const URL_WEBSITE = new URL(import.meta.env.PUBLIC_WEBSITE_URL ?? "https://stillwind.dev/")
  .href;

export const URL_DOCS = new URL("docs/", URL_WEBSITE).href;
