import { build } from "esbuild";

/**
 * Bundles React Refresh for emitFullSignatures mode, which requires no Node crypto.
 *
 * @param entry - Resolved React Refresh Babel plugin entry.
 * @param outfile - Generated ESM file that the build worker imports from the static site.
 */
export async function bundleRefresh(entry: string, outfile: string): Promise<void> {
  await build({
    entryPoints: [entry],
    outfile,
    bundle: true,
    format: "esm",
    platform: "browser",
    define: {
      "process.env.NODE_ENV": '"development"',
    },
    minify: true,
    plugins: [
      {
        name: "full-refresh-signatures",
        setup(plugin) {
          plugin.onResolve({ filter: /^crypto$/ }, () => ({
            path: "crypto",
            namespace: "full-signatures",
          }));
          plugin.onLoad(
            {
              filter: /.*/,
              namespace: "full-signatures",
            },
            () => ({
              contents:
                'export function createHash() { throw new Error("Browser React Refresh requires emitFullSignatures: true"); }',
            }),
          );
        },
      },
    ],
  });
}
