import type { Plugin } from "vite";

/** Filters repeated static files for Fresh's move operation without changing the emitted manifest. */
export function registrationManifest(source: string): string {
  const manifest = JSON.parse(source) as Record<string, { assets?: string[]; css?: string[] }>;
  const registered = new Set<string>();
  for (const entry of Object.values(manifest)) {
    for (const field of ["assets", "css"] as const) {
      entry[field] = entry[field]?.filter((file) => {
        if (registered.has(file)) {
          return false;
        }
        registered.add(file);
        return true;
      });
    }
  }
  return JSON.stringify(manifest);
}

/** Passes unique static files to Fresh while retaining the complete manifest for route CSS. */
export function registrationHooks(): Plugin {
  return {
    name: "stillwind:fresh-static-files",
    apply: "build",
    configResolved(config) {
      const entry = config.plugins.find((plugin) => plugin.name === "fresh:server_entry");
      const hook = entry?.writeBundle;
      if (!entry || !hook) {
        throw new Error("The Stillwind Fresh integration requires the Fresh Vite plugin");
      }
      const original = typeof hook === "function" ? hook : hook.handler;
      entry.writeBundle = {
        ...(typeof hook === "object" ? hook : {}),
        handler(options, bundle) {
          const manifest = bundle[".vite/manifest.json"];
          const registration =
            manifest?.type === "asset" && typeof manifest.source === "string"
              ? {
                  ...bundle,
                  [manifest.fileName]: {
                    ...manifest,
                    source: registrationManifest(manifest.source),
                  },
                }
              : bundle;
          return original.call(this, options, registration);
        },
      };
    },
  };
}
