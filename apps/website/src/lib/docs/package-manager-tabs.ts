import { initTabs } from "@cloudflare/nimbus-docs/client";

import { packageManagerStorageKey } from "./package-managers";

/**
 * Starts the package manager tabs on the page. Selecting a tab selects the same package manager in
 * every block, stores it for later pages, and sets `data-package-manager` on the document root,
 * which the stylesheet uses to show the selected commands.
 */
export function mountPackageManagerTabs(): void {
  for (const container of document.querySelectorAll<HTMLElement>("[data-package-managers]")) {
    const tabs = container.querySelectorAll<HTMLElement>("[role=tab]");
    initTabs({
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
  }
}
