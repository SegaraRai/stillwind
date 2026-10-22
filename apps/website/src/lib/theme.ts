/**
 * Connects a button in a live document to the shared theme preference.
 * Returns a disposer that releases click and storage listeners.
 */
export function mountThemeControl(button: HTMLButtonElement): () => void {
  const document = button.ownerDocument;
  const view = document.defaultView!;
  const root = document.documentElement;
  const storageKey = "stillwind:theme";
  const preferences = ["system", "light", "dark"] as const;
  type Preference = (typeof preferences)[number];
  const normalize = (value: string | null | undefined): Preference =>
    value === "light" || value === "dark" ? value : "system";
  const update = (preference: Preference) => {
    root.dataset.theme = preference;
    const next = preferences[(preferences.indexOf(preference) + 1) % preferences.length]!;
    const label = `Color theme: ${preference}. Switch to ${next}.`;
    button.setAttribute("aria-label", label);
    button.title = label;
    const status = button.querySelector<HTMLElement>("[data-theme-status]");
    if (status) {
      status.textContent = `Color theme: ${preference}`;
    }
  };
  const toggle = () => {
    const current = normalize(root.dataset.theme);
    const next = preferences[(preferences.indexOf(current) + 1) % preferences.length]!;
    update(next);
    try {
      if (next === "system") {
        view.localStorage.removeItem(storageKey);
      } else {
        view.localStorage.setItem(storageKey, next);
      }
    } catch {}
  };
  const synchronize = (event: StorageEvent) => {
    if (event.key === storageKey || event.key === null) {
      update(normalize(event.newValue));
    }
  };
  update(normalize(root.dataset.theme));
  button.hidden = false;
  button.addEventListener("click", toggle);
  view.addEventListener("storage", synchronize);
  return () => {
    button.removeEventListener("click", toggle);
    view.removeEventListener("storage", synchronize);
  };
}
