import { searchUtilities, type UtilityCatalog } from "./search";

export async function mountUtilityExplorer(root: HTMLElement) {
  const form = root.querySelector<HTMLFormElement>("form")!;
  const input = root.querySelector<HTMLInputElement>("[name=q]")!;
  const select = root.querySelector<HTMLSelectElement>("[name=mode]")!;
  const status = root.querySelector<HTMLElement>("[role=status]")!;
  const results = root.querySelector<HTMLElement>("[data-utility-results]")!;
  const view = root.ownerDocument.defaultView!;
  const params = new URL(view.location.href).searchParams;
  input.value = params.get("q") ?? "p-4";
  select.value = params.get("mode") === "css" ? "css" : "utility";
  try {
    const response = await fetch(root.dataset.catalog!);
    if (!response.ok) {
      throw new Error(`Catalog request failed: ${response.status}`);
    }
    const catalog = (await response.json()) as UtilityCatalog;
    const update = (save: boolean) => {
      const mode = select.value === "css" ? "css" : "utility";
      const answer = searchUtilities(catalog.records, mode, input.value);
      const fragment = root.ownerDocument.createDocumentFragment();
      for (const record of answer.records) {
        const card = root.ownerDocument.createElement("section");
        card.className = "utility-result";
        const heading = root.ownerDocument.createElement("h3");
        const name = root.ownerDocument.createElement("code");
        name.textContent = record.name;
        heading.append(name);
        const pre = root.ownerDocument.createElement("pre");
        const code = root.ownerDocument.createElement("code");
        code.textContent = record.css || "Marker class; no utility declarations.";
        pre.append(code);
        card.append(heading, pre);
        fragment.append(card);
      }
      results.replaceChildren(fragment);
      status.textContent = `${answer.total.toLocaleString("en")} matches. Showing ${answer.records.length}.`;
      if (save) {
        const url = new URL(view.location.href);
        url.searchParams.set("mode", mode);
        if (input.value) {
          url.searchParams.set("q", input.value);
        } else {
          url.searchParams.delete("q");
        }
        view.history.replaceState(null, "", url);
      }
    };
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      update(true);
    });
    input.addEventListener("input", () => update(true));
    select.addEventListener("change", () => update(true));
    view.addEventListener("popstate", () => {
      const query = new URL(view.location.href).searchParams;
      input.value = query.get("q") ?? "p-4";
      select.value = query.get("mode") === "css" ? "css" : "utility";
      update(false);
    });
    update(false);
  } catch {
    status.textContent =
      "Could not load the catalog. The examples below and the Markdown reference remain available.";
  }
}
