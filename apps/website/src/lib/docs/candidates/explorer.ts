import { applyStyle, style } from "stillwind/vanilla";

import { highlightCss } from "./highlight";
import { matchCaption, type CandidateLookup, type CandidateMatch } from "./lookup";
import type { LookupReply, LookupRequest } from "./worker";

/**
 * Connects the explorer markup in `root` to a lookup worker. The input and its `q` URL parameter
 * stay in sync. Without cross-origin isolation, the server-rendered results stay in place.
 * Returns a disposer that stops the worker and removes the listeners.
 */
export function mountCandidateExplorer(root: HTMLElement): () => void {
  const document = root.ownerDocument;
  const view = document.defaultView!;
  const form = root.querySelector<HTMLFormElement>("form")!;
  const input = root.querySelector<HTMLInputElement>("[name=q]")!;
  const status = root.querySelector<HTMLElement>("[role=status]")!;
  const results = root.querySelector<HTMLElement>("[data-candidate-results]")!;
  if (!view.crossOriginIsolated) {
    status.textContent =
      "This browser cannot run the compiler on this page. The results show an example.";
    return () => {};
  }

  const abort = new view.AbortController();
  const worker = new Worker(new URL("./worker.ts", import.meta.url), {
    type: "module",
  });
  let latest = 0;
  status.textContent = "Loading the compiler…";

  const render = (lookup: CandidateLookup, text: string) => {
    const cards = lookup.matches.map((match) => {
      const card = document.createElement("section");
      applyStyle(card, style("candidate-result"));
      const header = document.createElement("div");
      applyStyle(header, style("candidate-result-header"));
      const heading = document.createElement("h3");
      applyStyle(heading, style("candidate-result-title"));
      const name = document.createElement("code");
      name.textContent = match.candidate;
      heading.append(name);
      header.append(heading);
      const caption = matchCaption(match);
      if (caption) {
        const from = document.createElement("p");
        applyStyle(from, style("candidate-result-caption"));
        from.textContent = caption;
        header.append(from);
      }
      const pre = document.createElement("pre");
      applyStyle(pre, style("candidate-result-code"));
      const code = document.createElement("code");
      if (match.css) {
        code.innerHTML = highlightCss(match.css);
      } else {
        code.textContent = "Adds a marker class and no declarations.";
      }
      pre.append(code);
      card.append(header, pre);
      return card;
    });
    results.replaceChildren(...cards);
    const count = (source: CandidateMatch["source"]) =>
      lookup.matches.filter((match) => match.source === source).length;
    const prefixes = count("prefix");
    const variants = count("variant");
    const partial = [
      ...(variants < lookup.variantTotal
        ? [`Showing ${variants} of ${lookup.variantTotal} variants that match “${text.trim()}”.`]
        : []),
      ...(prefixes < lookup.prefixTotal
        ? [
            `Showing ${prefixes} of ${lookup.prefixTotal} utilities that start with “${text.trim()}”.`,
          ]
        : []),
    ];
    status.textContent = !text.trim()
      ? "Type a utility, variant, or CSS declarations."
      : lookup.matches.length === 0
        ? `No utility, variant, or CSS declaration matches “${text.trim()}”.`
        : partial.length > 0
          ? partial.join(" ")
          : `${lookup.matches.length} ${lookup.matches.length === 1 ? "match" : "matches"}.`;
  };
  worker.addEventListener("message", ({ data }: MessageEvent<LookupReply>) => {
    if (data.id !== latest) {
      return;
    }
    if ("error" in data) {
      status.textContent = `The compiler failed: ${data.error}`;
    } else {
      render(data.lookup, input.value);
    }
  });
  worker.addEventListener("error", () => {
    status.textContent = "Could not load the compiler. The results show an example.";
  });

  const request = (save: boolean) => {
    latest += 1;
    worker.postMessage({
      id: latest,
      input: input.value,
    } satisfies LookupRequest);
    if (save) {
      const url = new URL(view.location.href);
      if (input.value) {
        url.searchParams.set("q", input.value);
      } else {
        url.searchParams.delete("q");
      }
      view.history.replaceState(view.history.state, "", url);
    }
  };
  const { signal } = abort;
  form.addEventListener(
    "submit",
    (event) => {
      event.preventDefault();
    },
    {
      signal,
    },
  );
  input.addEventListener(
    "input",
    () => {
      request(true);
    },
    {
      signal,
    },
  );
  for (const example of root.querySelectorAll<HTMLButtonElement>("[data-example]")) {
    example.addEventListener(
      "click",
      () => {
        input.value = example.dataset.example!;
        input.focus();
        request(true);
      },
      { signal },
    );
  }
  view.addEventListener(
    "popstate",
    () => {
      input.value = new URL(view.location.href).searchParams.get("q") ?? "";
      request(false);
    },
    {
      signal,
    },
  );
  const shared = new URL(view.location.href).searchParams.get("q");
  if (shared !== null && shared !== input.value) {
    // The server-rendered results belong to the initial input.
    input.value = shared;
    results.replaceChildren();
  }
  request(false);

  return () => {
    abort.abort();
    worker.terminate();
  };
}
