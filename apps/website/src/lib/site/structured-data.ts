import type { Breadcrumb } from "@cloudflare/nimbus-docs/types";

/**
 * Serializes JSON-LD for an inline script element.
 *
 * @param data The JSON-LD object.
 * @returns JSON text without `<`, so a string value cannot close the script element.
 */
export function serializeStructuredData(data: object): string {
  return JSON.stringify(data).replaceAll("<", "\\u003c");
}

/**
 * Creates a `BreadcrumbList` from the breadcrumbs of a page.
 *
 * @param breadcrumbs The trail from the site root to the page.
 * @param site The origin that resolves site-relative breadcrumb URLs.
 * @returns JSON-LD with absolute URLs. Sidebar groups without a page have no URL, so they are skipped.
 */
export function breadcrumbList(breadcrumbs: readonly Breadcrumb[], site: URL) {
  return {
    "@context": "https://schema.org",
    "@type": "BreadcrumbList",
    itemListElement: breadcrumbs
      .filter(({ href }) => href !== undefined)
      .map(({ label, href }, index) => ({
        "@type": "ListItem",
        position: index + 1,
        name: label,
        item: new URL(href!, site).href,
      })),
  };
}
