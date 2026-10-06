import { createMemoryAnalysisWorkspace } from "@stillwind/analysis/memory";
import { startBrowserLanguageServer } from "@stillwind/language-server/browser";

startBrowserLanguageServer(self as unknown as Parameters<typeof startBrowserLanguageServer>[0], {
  openWorkspaces: async ({ workspaceFolders }) => {
    const { compileSource, __napiBindingTarget } = await import("@stillwind/native");
    if (__napiBindingTarget !== "wasm32-wasi" || workspaceFolders[0] !== "inmemory://play/") {
      throw new Error("Expected a browser WASM compiler and an in-memory workspace");
    }
    return {
      workspaces: new Map([
        [
          "play",
          createMemoryAnalysisWorkspace({
            config: "inmemory://play/stillwind.config.ts",
            includes: (uri) => uri.endsWith(".tsx"),
            compile: async ({ uri, documents }) => {
              const output = compileSource({
                source: documents.get(uri) ?? "",
                moduleId: uri,
                uri,
                sourceMapSource: uri,
                language: "tsx",
                moduleKind: "unambiguous",
                css: {
                  content:
                    "@utility red{color:red}@stillwind-slot utilities;@stillwind{unknown-utility:deny;}",
                  name: "theme.css",
                  uri: "inmemory://play/theme.css",
                  sourceMapSource: "inmemory://play/theme.css",
                },
              });
              return {
                status: "complete",
                reports: output.reports,
              };
            },
          }),
        ],
      ]),
    };
  },
  onExit: () => self.close(),
});
