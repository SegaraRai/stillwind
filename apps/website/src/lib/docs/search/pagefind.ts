import { searchIndex } from "virtual:nimbus/config";

/**
 * One documentation page that matches a query.
 */
export interface SearchResult {
  title: string;
  url: string;
  /** Pagefind excerpt as HTML. Pagefind escapes the page text and wraps matched words in `<mark>`. */
  excerpt: string;
  /** Sections of the page that match, in page order. Each URL links to the section heading. */
  sections: { title: string; url: string }[];
}

interface PagefindResultData {
  url: string;
  excerpt: string;
  meta: { title?: string };
  sub_results: { title: string; url: string }[];
}

interface Pagefind {
  options(options: { baseUrl: string }): Promise<void>;
  init(): Promise<void>;
  search(query: string): Promise<{ results: { data(): Promise<PagefindResultData> }[] }>;
}

let loading: Promise<Pagefind> | undefined;

/**
 * Loads the Pagefind bundle that Nimbus writes during a production build. The returned promise
 * rejects when the bundle cannot load, which always happens in development. After a rejection,
 * the next call tries again.
 */
export function loadPagefind(): Promise<Pagefind> {
  loading ??= (async () => {
    const pagefind = (await import(
      /* @vite-ignore */ new URL(`${searchIndex.bundle}pagefind.js`, location.origin).href
    )) as Pagefind;
    await pagefind.options({ baseUrl: import.meta.env.BASE_URL });
    await pagefind.init();
    return pagefind;
  })();
  loading.catch(() => {
    loading = undefined;
  });
  return loading;
}

/**
 * Returns at most `limit` pages that match `query`, with the best match first.
 * Rejects when Pagefind cannot load or the search fails.
 */
export async function searchDocs(query: string, limit = 10): Promise<SearchResult[]> {
  const pagefind = await loadPagefind();
  const { results } = await pagefind.search(query);
  const pages = await Promise.all(results.slice(0, limit).map((result) => result.data()));
  return pages.map((page) => ({
    title: page.meta.title ?? page.url,
    url: withoutTrailingSlash(page.url),
    excerpt: page.excerpt,
    sections: page.sub_results
      .filter((section) => section.url !== page.url)
      .map((section) => ({ title: section.title, url: withoutTrailingSlash(section.url) })),
  }));
}

/**
 * Removes the slash that Pagefind keeps from `index.html` paths, such as `/docs/cli/#usage`,
 * because the site serves pages without a trailing slash. The root path stays `/`.
 */
function withoutTrailingSlash(url: string): string {
  return url.replace(/(?<=.)\/(?=#|$)/, "");
}
