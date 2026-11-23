import { applyStyle, style } from "stillwind/vanilla";

import { loadPagefind, searchDocs, type SearchResult } from "./pagefind";

/** Delay in milliseconds between the last keystroke and the search. */
const DEBOUNCE = 150;

const option = style(
  "block px-3 py-2 rounded-control cursor-pointer hover:bg-surface-muted data-highlighted:bg-surface-muted",
);
const optionTitle = style("block truncate text-foreground text-small font-medium");
const optionExcerpt = style("line-clamp-2 mbs-1 text-caption text-muted search-excerpt");
const optionSections = style("grid mbs-1 ps-3 border-border border-s");
const optionSection = style(
  "block truncate px-2 py-1 text-caption text-muted rounded-control cursor-pointer hover:text-foreground hover:bg-surface-muted data-highlighted:text-foreground data-highlighted:bg-surface-muted",
);

/** Detail of the `search:open` event. */
interface SearchOpenDetail {
  /** Query that replaces the input value. When this is missing, the dialog keeps the previous input. */
  query?: string;
}

/**
 * Connects the search dialog markup in `dialog` to Pagefind. Typing searches after a short
 * delay, the arrow keys move through the results, and Enter opens the highlighted result.
 * Returns a disposer that removes the listeners and cancels a pending search.
 */
export function mountSearchDialog(dialog: HTMLDialogElement): () => void {
  const document = dialog.ownerDocument;
  const input = dialog.querySelector<HTMLInputElement>("[data-search-input]")!;
  const status = dialog.querySelector<HTMLElement>("[data-search-status]")!;
  const listbox = dialog.querySelector<HTMLElement>("[data-search-results]")!;
  const abort = new AbortController();
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Increases with each query, so a slower earlier search cannot replace newer results.
  let latest = 0;
  let active = -1;
  let optionCount = 0;

  const options = () => [...listbox.querySelectorAll<HTMLElement>("[role=option]")];

  const highlight = (index: number) => {
    const items = options();
    active = items.length === 0 ? -1 : Math.max(0, Math.min(index, items.length - 1));
    items.forEach((item, position) => {
      const selected = position === active;
      item.toggleAttribute("data-highlighted", selected);
      item.setAttribute("aria-selected", String(selected));
    });
    if (active < 0) {
      input.removeAttribute("aria-activedescendant");
      return;
    }
    input.setAttribute("aria-activedescendant", items[active]!.id);
    items[active]!.scrollIntoView({ block: "nearest" });
  };

  const navigate = (href: string) => {
    dialog.close();
    document.defaultView?.location.assign(href);
  };

  const show = (message: string, results: readonly SearchResult[] = []) => {
    status.textContent = message;
    status.toggleAttribute("data-results-present", results.length > 0);
    listbox.replaceChildren(...results.map(renderResult));
    // An empty listbox is invalid, so the listbox is hidden while it has no options.
    listbox.hidden = results.length === 0;
    input.setAttribute("aria-expanded", String(results.length > 0));
    active = -1;
    input.removeAttribute("aria-activedescendant");
  };

  const renderResult = (result: SearchResult) => {
    const group = document.createElement("div");
    const item = document.createElement("div");
    item.id = `search-result-${optionCount++}`;
    item.setAttribute("role", "option");
    item.setAttribute("aria-selected", "false");
    item.dataset.href = result.url;
    applyStyle(item, option);
    const title = document.createElement("span");
    title.textContent = result.title;
    applyStyle(title, optionTitle);
    item.append(title);
    if (result.excerpt) {
      const excerpt = document.createElement("p");
      applyStyle(excerpt, optionExcerpt);
      excerpt.innerHTML = result.excerpt;
      item.append(excerpt);
    }
    item.addEventListener("click", () => navigate(result.url));
    group.append(item);
    if (result.sections.length > 0) {
      const sections = document.createElement("div");
      applyStyle(sections, optionSections);
      for (const section of result.sections.slice(0, 3)) {
        const sub = document.createElement("div");
        sub.id = `search-result-${optionCount++}`;
        sub.setAttribute("role", "option");
        sub.setAttribute("aria-selected", "false");
        sub.dataset.href = section.url;
        sub.textContent = section.title;
        applyStyle(sub, optionSection);
        sub.addEventListener("click", () => navigate(section.url));
        sections.append(sub);
      }
      group.append(sections);
    }
    return group;
  };

  const search = async (query: string) => {
    const id = ++latest;
    if (!query) {
      show("Type to search the documentation.");
      return;
    }
    status.removeAttribute("data-results-present");
    status.textContent = "Searching…";
    try {
      const results = await searchDocs(query);
      if (id === latest) {
        show(
          results.length > 0
            ? `${results.length} ${results.length === 1 ? "result" : "results"} found.`
            : "No results found.",
          results,
        );
      }
    } catch {
      if (id === latest) {
        show(
          import.meta.env.DEV
            ? "Search is available after a production build."
            : "Search could not load. Check your connection and reload the page.",
        );
      }
    }
  };

  input.addEventListener(
    "input",
    () => {
      clearTimeout(timer);
      timer = setTimeout(() => void search(input.value.trim()), DEBOUNCE);
    },
    { signal: abort.signal },
  );
  input.addEventListener(
    "keydown",
    (event) => {
      if (event.key === "ArrowDown") {
        highlight(active + 1);
      } else if (event.key === "ArrowUp") {
        highlight(active - 1);
      } else if (event.key === "Enter" && active >= 0) {
        const href = options()[active]?.dataset.href;
        if (href) {
          navigate(href);
        }
      } else {
        return;
      }
      event.preventDefault();
    },
    { signal: abort.signal },
  );
  dialog.addEventListener(
    "click",
    (event) => {
      // A click on the backdrop targets the dialog itself.
      if (event.target === dialog || (event.target as Element).closest("a")) {
        dialog.close();
      }
    },
    { signal: abort.signal },
  );

  dialog.dataset.searchReady = "";
  dialog.addEventListener(
    "search:open",
    (event) => {
      const { query } = (event as CustomEvent<SearchOpenDetail>).detail;
      if (query !== undefined) {
        input.value = query;
        clearTimeout(timer);
        void search(query.trim());
      }
      if (!dialog.open) {
        dialog.showModal();
      }
      input.select();
      void loadPagefind().catch(() => {});
    },
    { signal: abort.signal },
  );

  return () => {
    abort.abort();
    clearTimeout(timer);
    latest++;
  };
}

/**
 * Opens the search dialog from any element with `data-search-trigger` and from Ctrl+K, or Cmd+K
 * on macOS. When the trigger has `data-search-query`, the dialog searches for that value; otherwise
 * it keeps the previous input. Call it once per document. The listeners stay on `document`, which
 * survives client-side navigation, so they open the dialog of the current page.
 */
export function bindSearchShortcuts(document: Document): void {
  const open = (query?: string) =>
    document.querySelector("dialog[data-search-dialog][data-search-ready]")?.dispatchEvent(
      new CustomEvent<SearchOpenDetail>("search:open", {
        detail: {
          query,
        },
      }),
    );

  document.addEventListener("click", (event) => {
    const trigger = (event.target as Element).closest<HTMLElement>("[data-search-trigger]");
    if (trigger) {
      open(trigger.dataset.searchQuery);
    }
  });
  document.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      const dialog = document.querySelector<HTMLDialogElement>("dialog[data-search-dialog]");
      if (dialog?.open) {
        dialog.close();
      } else {
        open();
      }
    }
  });
}

/** Shows ⌘ instead of Ctrl in the shortcut hint of `trigger` on macOS. */
export function mountSearchTrigger(trigger: HTMLElement): () => void {
  const platform =
    (navigator as Navigator & { userAgentData?: { platform: string } }).userAgentData?.platform ??
    navigator.userAgent;
  if (/mac|iphone|ipad|ipod/i.test(platform)) {
    trigger.setAttribute("aria-keyshortcuts", "Meta+K");
    const shortcut = trigger.querySelector("[data-shortcut-key]");
    if (shortcut) {
      shortcut.textContent = "⌘";
    }
  }
  return () => {};
}
