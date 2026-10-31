import { PassThrough } from "node:stream";

import type { AnalysisWorkspaceService } from "@stillwind/analysis";
import { BrowserAnalysisWorkspace } from "@stillwind/analysis/browser";
import { expect, test } from "vite-plus/test";
import type {
  CodeAction,
  ColorInformation,
  ColorPresentation,
  CompletionItem,
  DocumentLink,
  Hover,
  InitializeResult,
  Location,
} from "vscode-languageserver";
import { TextDocument } from "vscode-languageserver-textdocument";
import {
  createConnection,
  StreamMessageReader,
  StreamMessageWriter,
} from "vscode-languageserver/node";

import { createLanguageServer } from "./server.ts";

/**
 * Creates a service that reports one result for each document in its scope.
 *
 * @param covers - Document URIs that this service analyzes.
 * @returns A service with no editor queries.
 */
function coveringService(covers: readonly string[]): AnalysisWorkspaceService {
  return {
    update: () => {},
    closeDocument: () => {},
    close: async () => {},
    analyze: async (uri) =>
      covers.includes(uri)
        ? [
            {
              project: 0,
              config: "config",
              result: {
                entry: uri,
                revision: 0,
                status: "complete",
                reports: [],
              },
            },
          ]
        : [],
  };
}

test.each([
  {
    name: "opens a configuration for an uncovered document",
    eager: [],
    calls: 1,
  },
  {
    name: "keeps a document that an open service covers",
    eager: ["file:///workspace/apps/web/App.tsx"],
    calls: 0,
  },
])("LSP discovery $name", async ({ eager, calls }) => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/apps/web/App.tsx";
  const discovered: string[] = [];
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map(eager.length ? [["root", coveringService(eager)]] : []),
    }),
    discoverWorkspace: async (document) => {
      discovered.push(document);
      return {
        workspaces: new Map([["app", coveringService([document])]]),
      };
    },
  });
  server.listen();
  client.listen();
  try {
    const initialized = await client.sendRequest<InitializeResult>("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
    });
    expect(initialized.capabilities.hoverProvider).toBe(true);
    await client.sendNotification("initialized", {});
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "typescriptreact",
        version: 1,
        text: "",
      },
    });
    await new Promise((resolve) => setTimeout(resolve, 200));
    await service.ready();
    expect(discovered).toHaveLength(calls);
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
    input.destroy();
    output.destroy();
  }
});

test("LSP definition requests navigate CSS asset references using original UTF-16 ranges", async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/theme.css";
  const text =
    "@theme asset {/* 🦀 */--image:url(data:image/svg+xml,asset);}\n.a{mask-image:var(--image);}";
  const document = TextDocument.create(uri, "css", 1, text);
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map([
        [
          "config",
          new BrowserAnalysisWorkspace({
            config: "config",
            projects: [
              {
                root: "/workspace",
                css: "theme.css",
                files: ["*.tsx"],
                typeFacts: false,
              },
            ],
            files: new Map([["/workspace/theme.css", text]]),
          }),
        ],
      ]),
    }),
  });
  server.listen();
  client.listen();
  try {
    const initialized = await client.sendRequest<InitializeResult>("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
    });
    expect(initialized.capabilities.definitionProvider).toBe(true);
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version: 1,
        text,
      },
    });
    const definitions = await client.sendRequest<Location[]>("textDocument/definition", {
      textDocument: {
        uri,
      },
      position: document.positionAt(text.lastIndexOf("--image") + 4),
    });
    const start = text.indexOf("--image");
    expect(definitions).toEqual([
      {
        uri,
        range: {
          start: document.positionAt(start),
          end: document.positionAt(start + "--image".length),
        },
      },
    ]);
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
  }
});

test.each([false, true])(
  "LSP variant snippets respect client support (%s) and continue completion",
  async (snippetSupport) => {
    const input = new PassThrough();
    const output = new PassThrough();
    const server = createConnection(
      new StreamMessageReader(input),
      new StreamMessageWriter(output),
    );
    const client = createConnection(
      new StreamMessageReader(output),
      new StreamMessageWriter(input),
    );
    const uri = "file:///workspace/entry.tsx";
    const text = 'import {style} from "stillwind/react"; const card=style("hover:");';
    const document = TextDocument.create(uri, "typescriptreact", 1, text);
    const service = createLanguageServer(server, {
      openWorkspaces: async () => ({
        workspaces: new Map([
          [
            "config",
            new BrowserAnalysisWorkspace({
              config: "config",
              projects: [
                {
                  root: "/workspace",
                  css: "theme.css",
                  files: ["*.tsx"],
                  typeFacts: false,
                },
              ],
              files: new Map([
                ["/workspace/entry.tsx", text],
                [
                  "/workspace/theme.css",
                  `@utility block { display:block; }
@custom-variant hover (&:hover);
@stillwind-variant nth-* (value: [nth]) { &:--value(":nth-child({value})") { @slot; } }`,
                ],
              ]),
            }),
          ],
        ]),
      }),
    });
    server.listen();
    client.listen();
    try {
      await client.sendRequest("initialize", {
        rootUri: "file:///workspace",
        capabilities: {
          textDocument: {
            completion: {
              completionItem: {
                snippetSupport,
              },
            },
          },
        },
      });
      await client.sendNotification("textDocument/didOpen", {
        textDocument: {
          uri,
          languageId: document.languageId,
          version: 1,
          text,
        },
      });
      const complete = (offset: number) =>
        client.sendRequest<CompletionItem[]>("textDocument/completion", {
          textDocument: {
            uri,
          },
          position: document.positionAt(offset),
        });
      const items = await complete(text.indexOf("hover:") + "hover:".length);
      expect(items.some((item) => item.label === "hover:hover:")).toBe(false);
      const arbitrary = items.find((item) => item.label === "hover:nth-[]:");
      if (snippetSupport) {
        expect(arbitrary).toMatchObject({
          insertTextFormat: 2,
          textEdit: {
            newText: "hover:nth-[${1}]:${0}",
          },
          command: {
            command: "editor.action.triggerSuggest",
          },
        });
      } else {
        expect(arbitrary).toBeUndefined();
        expect(items.every((item) => item.insertTextFormat !== 2)).toBe(true);
      }
      const block = items.find((item) => item.label === "hover:block")!;
      expect(block.detail).toBeUndefined();
      const resolved = await client.sendRequest<CompletionItem>("completionItem/resolve", block);
      expect(resolved.detail).toBe("display: block;");
      expect(JSON.stringify(resolved.documentation)).toContain("&:hover");
      const variant = (await complete(text.indexOf("hover:"))).find(
        (item) => item.label === "hover:",
      )!;
      expect(variant.command).toMatchObject({
        command: "editor.action.triggerSuggest",
      });
      expect(
        (await client.sendRequest<CompletionItem>("completionItem/resolve", variant)).detail,
      ).toContain("&:hover");
      await client.sendNotification("textDocument/didChange", {
        textDocument: {
          uri,
          version: 2,
        },
        contentChanges: [
          {
            text: text.replace("hover:", ""),
          },
        ],
      });
      const stale = await client.sendRequest<CompletionItem>("completionItem/resolve", resolved);
      expect(stale.detail).toBeUndefined();
      expect(stale.documentation).toBeUndefined();
    } finally {
      await service.dispose();
      client.dispose();
      server.dispose();
      input.destroy();
      output.destroy();
    }
  },
);

test("LSP plugin navigation preserves Unicode ranges and omits conflicting project targets", async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/theme.css";
  const text =
    '/* 日本語 🦀 */ @plugin "./local.mjs"; @plugin "demo-plugin"; @plugin "missing"; @plugin "de';
  const document = TextDocument.create(uri, "css", 1, text);
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map([
        [
          "config",
          new BrowserAnalysisWorkspace({
            config: "config",
            projects: [
              {
                root: "/workspace",
                css: "theme.css",
                files: ["*.tsx"],
                typeFacts: false,
              },
              {
                root: "/workspace",
                css: "theme.css",
                files: ["*.tsx"],
                typeFacts: false,
                plugins: [
                  {
                    name: "demo-plugin",
                    css: () => "",
                  },
                ],
              },
            ],
            files: new Map([
              ["/workspace/theme.css", text],
              ["/workspace/local.mjs", "throw new Error('must not run');"],
              [
                "/workspace/package.json",
                JSON.stringify({
                  dependencies: {
                    "demo-plugin": "1",
                  },
                }),
              ],
              [
                "/workspace/node_modules/demo-plugin/package.json",
                JSON.stringify({
                  main: "index.js",
                }),
              ],
              ["/workspace/node_modules/demo-plugin/index.js", "throw new Error('must not run');"],
            ]),
          }),
        ],
      ]),
    }),
  });
  server.listen();
  client.listen();
  try {
    const initialized = await client.sendRequest<InitializeResult>("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
    });
    expect(initialized.capabilities.documentLinkProvider).toEqual({
      resolveProvider: false,
    });
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version: 1,
        text,
      },
    });
    const links = await client.sendRequest<DocumentLink[]>("textDocument/documentLink", {
      textDocument: {
        uri,
      },
    });
    const start = text.indexOf("./local.mjs");
    expect(links).toEqual([
      {
        range: {
          start: document.positionAt(start),
          end: document.positionAt(start + "./local.mjs".length),
        },
        target: "file:///workspace/local.mjs",
      },
    ]);
    const items = await client.sendRequest<CompletionItem[]>("textDocument/completion", {
      textDocument: {
        uri,
      },
      position: document.positionAt(text.length),
    });
    const item = items.find((item) => item.label === "demo-plugin")!;
    expect(item).toMatchObject({
      kind: 9,
      textEdit: {
        newText: 'demo-plugin"',
      },
    });
    expect((await client.sendRequest<CompletionItem>("completionItem/resolve", item)).detail).toBe(
      "Plugin module",
    );
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
    input.destroy();
    output.destroy();
  }
});

test("LSP completion settings expose registry metadata and invalidate prior CSS resolutions", async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/theme.css";
  const text = "@stillwind-utility example { display:block; }\n/* 日本語 🦀 */ @stillwind-";
  const document = TextDocument.create(uri, "css", 1, text);
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map([
        [
          "config",
          new BrowserAnalysisWorkspace({
            config: "config",
            projects: [
              {
                root: "/workspace",
                css: "theme.css",
                files: ["*.tsx"],
                typeFacts: false,
              },
            ],
            files: new Map([["/workspace/theme.css", text]]),
          }),
        ],
      ]),
    }),
  });
  server.listen();
  client.listen();
  const complete = () =>
    client.sendRequest<CompletionItem[]>("textDocument/completion", {
      textDocument: { uri },
      position: document.positionAt(text.length),
    });
  try {
    await client.sendRequest("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
      initializationOptions: {
        completion: {
          registryDirectives: true,
        },
      },
    });
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version: 1,
        text,
      },
    });
    const enabled = await complete();
    const registry = enabled.find((item) => item.label === "@stillwind-utility")!;
    expect(registry.detail).toBe("Registry authoring · Stillwind syntax");
    expect(registry.sortText).toEqual(expect.any(String));
    expect(registry.deprecated).not.toBe(true);
    const authoring = enabled.find((item) => item.label === "@stillwind-slot")!;
    await client.sendNotification("workspace/didChangeConfiguration", {
      settings: {
        stillwind: {
          completion: {
            registryDirectives: false,
          },
        },
      },
    });
    expect((await complete()).some((item) => item.label === "@stillwind-utility")).toBe(false);
    expect(
      (await client.sendRequest<CompletionItem>("completionItem/resolve", authoring)).documentation,
    ).toBeUndefined();
    const hover = await client.sendRequest<Hover>("textDocument/hover", {
      textDocument: { uri },
      position: document.positionAt(4),
    });
    expect(JSON.stringify(hover.contents)).toContain("Registry authoring");
    await client.sendNotification("workspace/didChangeConfiguration", {
      settings: {
        stillwind: {
          completion: {
            registryDirectives: true,
          },
        },
      },
    });
    const restored = (await complete()).find((item) => item.label === "@stillwind-utility")!;
    expect(
      (await client.sendRequest<CompletionItem>("completionItem/resolve", restored)).documentation,
    ).toMatchObject({
      kind: "markdown",
      value: expect.stringContaining("Registry authoring"),
    });
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
  }
});

test("CSS LSP requests complete cold incomplete buffers and resolve dialect and semantic hovers", async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/theme.css";
  const definitions = `@stillwind-theme-namespace --color-* [color];
@theme { --color-brand:red; }
@utility flex {display:flex;}
@utility bg-* {background-color:--value(--color-*);}
@custom-variant hover (&:hover);
`;
  let text = `${definitions}/* 日本語 🦀 */ .a { @apply fl`;
  let version = 1;
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map([
        [
          "config",
          new BrowserAnalysisWorkspace({
            config: "config",
            projects: [
              {
                root: "/workspace",
                files: ["*.tsx"],
                css: "theme.css",
                typeFacts: false,
              },
            ],
            files: new Map([["/workspace/theme.css", text]]),
          }),
        ],
      ]),
    }),
  });
  server.listen();
  client.listen();
  const position = (offset: number) =>
    TextDocument.create(uri, "css", version, text).positionAt(offset);
  const complete = () =>
    client.sendRequest<CompletionItem[]>("textDocument/completion", {
      textDocument: { uri },
      position: position(text.length),
    });
  const change = async (next: string) => {
    text = next;
    await client.sendNotification("textDocument/didChange", {
      textDocument: {
        uri,
        version: ++version,
      },
      contentChanges: [{ text }],
    });
  };
  try {
    await client.sendRequest("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
    });
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "css",
        version,
        text,
      },
    });
    const item = (await complete()).find((item) => item.label === "flex")!;
    expect(item.textEdit).toMatchObject({
      range: {
        start: position(text.length - 2),
        end: position(text.length),
      },
      newText: "flex",
    });
    const resolved = await client.sendRequest<CompletionItem>("completionItem/resolve", item);
    expect(JSON.stringify(resolved.documentation)).toContain("display: flex");
    await change(`${definitions}.a { @variant ho`);
    const variant = (await complete()).find((item) => item.label === "hover")!;
    expect(variant.textEdit).toMatchObject({ newText: "hover" });
    expect(
      JSON.stringify(
        (await client.sendRequest<CompletionItem>("completionItem/resolve", variant)).documentation,
      ),
    ).toContain("&:hover");
    expect(
      (await client.sendRequest<CompletionItem>("completionItem/resolve", resolved)).documentation,
    ).toBeUndefined();
    await change(`${definitions}.a { color:--theme(--color-br`);
    const token = (await complete()).find((item) => item.label === "--color-brand")!;
    expect(
      JSON.stringify(
        (await client.sendRequest<CompletionItem>("completionItem/resolve", token)).documentation,
      ),
    ).toContain("--color-brand: red");
    await change(`${definitions}.a { @stillwind-slot property-initials inh`);
    const slot = (await complete()).find((item) => item.label === "property-initials inherited")!;
    expect(
      JSON.stringify(
        (await client.sendRequest<CompletionItem>("completionItem/resolve", slot)).documentation,
      ),
    ).toContain("inherits: true");
    await change("@app");
    const directive = (await complete()).find((item) => item.label === "@apply")!;
    expect(directive.kind).toBe(14);
    const documentation = (
      await client.sendRequest<CompletionItem>("completionItem/resolve", directive)
    ).documentation;
    expect(JSON.stringify(documentation)).toContain("@apply utilities;");
    await change(`${definitions}.a { @apply hover:flex; color:var(--color-brand); }`);
    const hover = await client.sendRequest<Hover>("textDocument/hover", {
      textDocument: { uri },
      position: position(text.indexOf("hover:flex") + 8),
    });
    expect(JSON.stringify(hover.contents)).toContain("display: flex");
    const themeHover = await client.sendRequest<Hover>("textDocument/hover", {
      textDocument: { uri },
      position: position(text.lastIndexOf("--color-brand") + 6),
    });
    expect(JSON.stringify(themeHover.contents)).toContain("cascade");
    const references = await client.sendRequest<ColorInformation[]>("textDocument/documentColor", {
      textDocument: { uri },
    });
    expect(references).toHaveLength(1);
    const referenceEdit = await client.sendRequest<ColorPresentation[]>(
      "textDocument/colorPresentation",
      {
        textDocument: { uri },
        range: references[0].range,
        color: {
          red: 0,
          green: 0,
          blue: 1,
          alpha: 1,
        },
      },
    );
    expect(referenceEdit[0].textEdit?.newText).toBe("#0000ff");
    await change(`${definitions}/* 日本語 🦀 */ .a { @apply hover:bg-brand`);
    const colors = await client.sendRequest<ColorInformation[]>("textDocument/documentColor", {
      textDocument: { uri },
    });
    expect(colors[0].range.end).toEqual(position(text.length));
    const candidateEdit = await client.sendRequest<ColorPresentation[]>(
      "textDocument/colorPresentation",
      {
        textDocument: { uri },
        range: colors[0].range,
        color: {
          red: 0,
          green: 0,
          blue: 1,
          alpha: 1,
        },
      },
    );
    expect(candidateEdit[0].textEdit?.newText).toBe("hover:bg-[#0000ff]");
    await change(`${definitions}.a { @apply bg-brand flex; }`);
    const actions = await client.sendRequest<CodeAction[]>("textDocument/codeAction", {
      textDocument: { uri },
      range: {
        start: position(0),
        end: position(text.length),
      },
      context: {
        diagnostics: [],
        only: ["source.sort.stillwind"],
      },
    });
    expect(JSON.stringify(actions[0].edit)).toContain("flex bg-brand");
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
    input.destroy();
    output.destroy();
  }
});

test("standard LSP requests support lazy completion, hover, colors, and separate fix-all and sort actions", async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/entry.tsx";
  const source =
    'import {style} from "stillwind/react"; const card=style("p-[16px] flex block hover:bg-brand"); const frames=style.keyframes({from:"block"}); const transition=style.viewTransition({group:"duration-normal both",old:"block"}); declare const id:string; const dynamic=style(transition.for(id));';
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map([
        [
          "config",
          new BrowserAnalysisWorkspace({
            config: "config",
            projects: [
              {
                root: "/workspace",
                files: ["*.tsx"],
                css: "theme.css",
                typeFacts: false,
              },
            ],
            files: new Map([
              ["/workspace/entry.tsx", source],
              [
                "/workspace/theme.css",
                `@stillwind-theme-namespace --spacing-* [length] scale;
@theme { --spacing:.25rem; --color-brand:red; }
@utility p-* { padding:--value(--spacing-*, --spacing(), [length]); }
@utility block { display:block; } @utility flex { display:flex; }
@utility bg-* { background-color:--value(--color-*, [color]); }
@theme { --duration-normal:180ms; }
@stillwind-registry animation, view-transition {
  @utility both { animation-fill-mode:both; }
  @utility duration-* { animation-duration:--value(--duration-*); }
}
@variant hover { &:hover { @slot; } } @stillwind-slot utilities;`,
              ],
            ]),
          }),
        ],
      ]),
    }),
  });
  server.listen();
  client.listen();
  try {
    const initialized = await client.sendRequest<{
      /** Provider declarations returned by the LSP initialize response. */
      capabilities: {
        /** Whether the opened service supports hover queries. */
        hoverProvider: boolean;
        /** Source action kinds advertised alongside quick fixes. */
        codeActionProvider: {
          /** LSP kind strings that clients can request through context.only. */
          codeActionKinds: string[];
        };
      };
    }>("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
    });
    expect(initialized.capabilities).toMatchObject({
      hoverProvider: true,
      codeActionProvider: {
        codeActionKinds: expect.arrayContaining([
          "source.fixAll.stillwind",
          "source.sort.stillwind",
        ]),
      },
    });
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: "typescriptreact",
        version: 1,
        text: source,
      },
    });
    const position = {
      line: 0,
      character: source.indexOf("hover:bg-brand") + 8,
    };
    const hover = await client.sendRequest<Hover>("textDocument/hover", {
      textDocument: {
        uri,
      },
      position,
    });
    expect(JSON.stringify(hover.contents)).toContain("&:hover");
    const completions = await client.sendRequest<CompletionItem[]>("textDocument/completion", {
      textDocument: {
        uri,
      },
      position: {
        line: 0,
        character: source.indexOf("flex"),
      },
    });
    const item = completions.find((item) => item.label === "bg-brand")!;
    expect(item.documentation).toBeUndefined();
    const resolved = await client.sendRequest<CompletionItem>("completionItem/resolve", item);
    expect(JSON.stringify(resolved.documentation)).toContain("background-color");
    const animationItems = await client.sendRequest<CompletionItem[]>("textDocument/completion", {
      textDocument: {
        uri,
      },
      position: {
        line: 0,
        character: source.indexOf("duration-normal"),
      },
    });
    expect(animationItems.map((item) => item.label)).toEqual(
      expect.arrayContaining(["duration-normal", "both"]),
    );
    expect(animationItems.some((item) => item.label === "hover:")).toBe(false);
    const animation = await client.sendRequest<CompletionItem>(
      "completionItem/resolve",
      animationItems.find((item) => item.label === "duration-normal")!,
    );
    expect(JSON.stringify(animation.documentation)).toContain("::view-transition-group");
    expect(JSON.stringify(animation.documentation)).toContain("animation-duration:");
    const definition = await client.sendRequest<Hover>("textDocument/hover", {
      textDocument: {
        uri,
      },
      position: {
        line: 0,
        character: source.indexOf("style.keyframes"),
      },
    });
    expect(JSON.stringify(definition.contents)).toContain("@keyframes");
    expect(JSON.stringify(definition.contents)).toContain("__keyframes_frames");
    const dynamic = await client.sendRequest<Hover>("textDocument/hover", {
      textDocument: {
        uri,
      },
      position: {
        line: 0,
        character: source.indexOf("transition.for"),
      },
    });
    expect(JSON.stringify(dynamic.contents)).toContain("__transition_transition");
    expect(JSON.stringify(dynamic.contents)).toContain(
      "view-transition-name is computed at runtime from ` id `.",
    );
    const colors = await client.sendRequest<ColorInformation[]>("textDocument/documentColor", {
      textDocument: {
        uri,
      },
    });
    expect(colors).toHaveLength(1);
    const presentations = await client.sendRequest<ColorPresentation[]>(
      "textDocument/colorPresentation",
      {
        textDocument: {
          uri,
        },
        range: colors[0].range,
        color: {
          red: 0,
          green: 0,
          blue: 1,
          alpha: 1,
        },
      },
    );
    expect(presentations[0].textEdit?.newText).toBe("hover:bg-[#0000ff]");
    const actionParams = {
      textDocument: {
        uri,
      },
      range: {
        start: {
          line: 0,
          character: 0,
        },
        end: {
          line: 0,
          character: source.length,
        },
      },
      context: {
        diagnostics: [],
        only: ["source.fixAll.stillwind"],
      },
    };
    const fixes = await client.sendRequest<CodeAction[]>("textDocument/codeAction", actionParams);
    expect(fixes).toHaveLength(1);
    expect(JSON.stringify(fixes[0].edit)).toContain("p-4");
    expect(JSON.stringify(fixes[0].edit)).not.toContain("p-4 block");
    const sorts = await client.sendRequest<CodeAction[]>("textDocument/codeAction", {
      ...actionParams,
      context: {
        diagnostics: [],
        only: ["source.sort.stillwind"],
      },
    });
    expect(sorts).toHaveLength(1);
    expect(JSON.stringify(sorts[0].edit)).toContain("p-[16px]");
    await client.sendNotification("textDocument/didChange", {
      textDocument: {
        uri,
        version: 2,
      },
      contentChanges: [
        {
          text: source.replace("brand", "missing"),
        },
      ],
    });
    const stale = await client.sendRequest<CompletionItem>("completionItem/resolve", resolved);
    expect(stale.documentation).toBeUndefined();
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
    input.destroy();
    output.destroy();
  }
});

test("LSP marker completions use the class kind and resolve to their documentation", async () => {
  const input = new PassThrough();
  const output = new PassThrough();
  const server = createConnection(new StreamMessageReader(input), new StreamMessageWriter(output));
  const client = createConnection(new StreamMessageReader(output), new StreamMessageWriter(input));
  const uri = "file:///workspace/entry.tsx";
  const text = 'import {style} from "stillwind/react"; const value=style("car");';
  const document = TextDocument.create(uri, "typescriptreact", 1, text);
  const service = createLanguageServer(server, {
    openWorkspaces: async () => ({
      workspaces: new Map([
        [
          "config",
          new BrowserAnalysisWorkspace({
            config: "config",
            projects: [
              {
                root: "/workspace",
                css: "theme.css",
                files: ["*.tsx"],
                typeFacts: false,
              },
            ],
            files: new Map([
              ["/workspace/entry.tsx", text],
              ["/workspace/theme.css", "@utility card-title { .card & { color: red; } }"],
            ]),
          }),
        ],
      ]),
    }),
  });
  server.listen();
  client.listen();
  try {
    await client.sendRequest("initialize", {
      rootUri: "file:///workspace",
      capabilities: {},
    });
    await client.sendNotification("textDocument/didOpen", {
      textDocument: {
        uri,
        languageId: document.languageId,
        version: 1,
        text,
      },
    });
    const items = await client.sendRequest<CompletionItem[]>("textDocument/completion", {
      textDocument: {
        uri,
      },
      position: document.positionAt(text.indexOf('");')),
    });
    const marker = items.find((item) => item.label === "card")!;
    expect(marker.kind).toBe(7);
    expect(items.find((item) => item.label === "card-title")?.kind).toBe(12);
    const resolved = await client.sendRequest<CompletionItem>("completionItem/resolve", marker);
    expect(resolved).toMatchObject({
      detail: "marker class",
      documentation: {
        kind: "markdown",
        value:
          "Adds the marker class `card` to the element as written.\n\nSelectors in the `card-title` utility select this class.",
      },
    });
  } finally {
    await service.dispose();
    client.dispose();
    server.dispose();
    input.destroy();
    output.destroy();
  }
});
