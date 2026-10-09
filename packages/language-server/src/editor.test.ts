import { PassThrough } from "node:stream";

import { BrowserAnalysisWorkspace } from "@stillwind/analysis/browser";
import { expect, test } from "vite-plus/test";
import type {
  CodeAction,
  ColorInformation,
  ColorPresentation,
  CompletionItem,
  Hover,
} from "vscode-languageserver";
import {
  createConnection,
  StreamMessageReader,
  StreamMessageWriter,
} from "vscode-languageserver/node";

import { createLanguageServer } from "./index.ts";

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
                `@theme { --spacing:.25rem; --color-brand:red; }
@utility p-* { padding:--value(--spacing-*, --spacing(), [length]); }
@utility block { display:block; } @utility flex { display:flex; }
@utility bg-* { background-color:--value(--color-*, [color]); }
@theme { --duration-normal:180ms; }
@stillwind-animation-option both { animation-fill-mode:both; }
@stillwind-animation-option duration-* { animation-duration:--value(--duration-*); }
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
    expect(animationItems.some((item) => item.label === "block" || item.label === "hover:")).toBe(
      false,
    );
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
