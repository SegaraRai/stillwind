import { initTabs, mount } from "@cloudflare/nimbus-docs/client";

import { packageManagerStorageKey } from "./package-managers";

/**
 * Starts the package manager tabs on the current page and on each page that the client router
 * loads later. Selecting a tab selects the same package manager in every block, stores it for
 * later pages, and sets `data-package-manager` on the document root, which the stylesheet uses
 * to show the selected commands. Call it once.
 */
export function mountPackageManagerTabs(): void {
  mount("[data-package-managers]", (container) => {
    const tabs = container.querySelectorAll<HTMLElement>("[role=tab]");
    const instance = initTabs({
      container,
      tabSelector: "[role=tab]",
      panelSelector: "[role=tabpanel]",
      sync: {
        key: packageManagerStorageKey,
        getLabel: (tab) => tab.dataset.packageManager!,
      },
      onActivate: (index) => {
        document.documentElement.dataset.packageManager = tabs[index]!.dataset.packageManager;
      },
    });
    return () => instance.destroy();
  });
}
