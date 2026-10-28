import { expect, test, vi } from "vite-plus/test";
import type * as vscode from "vscode";

import { formatSource, registerFormatter } from "./formatter.ts";

test.each([
  ["javascript", "const x={hello:'world'}", 'const x = { hello: "world" };\n'],
  ["javascriptreact", "const x=<div a='b'/>", 'const x = <div a="b" />;\n'],
  ["typescript", "const x:number=1", "const x: number = 1;\n"],
  ["typescriptreact", "const x=<div a='b'/>", 'const x = <div a="b" />;\n'],
  ["css", ".x{color:red}", ".x {\n  color: red;\n}\n"],
  ["json", '{"x":1}', '{ "x": 1 }\n'],
  ["jsonc", '// keep comment\n{"x":1}', '// keep comment\n{ "x": 1 }\n'],
])("formats %s with the browser runtime", async (language, text, expected) => {
  const options = {
    insertSpaces: true,
    tabSize: 2,
  };
  const formatted = await formatSource(text, language, options);
  expect(formatted).toBe(expected);
  expect(await formatSource(formatted, language, options)).toBe(formatted);
});

test("respects indentation and preserves utility order and unsupported text", async () => {
  const options = {
    insertSpaces: false,
    tabSize: 4,
  };
  expect(
    await formatSource('function x(){return style("p-4 flex block")}', "javascript", options),
  ).toBe('function x() {\n\treturn style("p-4 flex block");\n}\n');
  expect(await formatSource("keep  whitespace", "plaintext", options)).toBe("keep  whitespace");
  await expect(formatSource("const =", "typescript", options)).rejects.toThrow();
});

test("the provider skips cancelled requests and discards edits for changed buffers", async () => {
  let provider: vscode.DocumentFormattingEditProvider | undefined;
  const replace = vi.fn(() => {
    expect.fail("A cancelled or changed document must not receive an edit");
  });
  const register: typeof vscode.languages.registerDocumentFormattingEditProvider = (
    _selector,
    registered,
  ) => {
    provider = registered;
    return { dispose() {} };
  };
  registerFormatter({
    languages: {
      registerDocumentFormattingEditProvider: register,
    },
    TextEdit: {
      replace,
    },
    commands: {
      executeCommand: async () => ({
        edits: [],
      }),
    },
  } as unknown as typeof vscode);
  const getText = vi.fn(() => "const x=1");
  const document = {
    version: 1,
    languageId: "typescript",
    uri: {
      path: "/workspace/entry.ts",
      toString: () => "file:///workspace/entry.ts",
    },
    getText,
  } as unknown as vscode.TextDocument;
  const token = {
    isCancellationRequested: true,
  } as vscode.CancellationToken;
  const options = {
    insertSpaces: true,
    tabSize: 2,
  };
  expect(await provider!.provideDocumentFormattingEdits(document, options, token)).toEqual([]);
  expect(getText).not.toHaveBeenCalled();
  const pending = provider!.provideDocumentFormattingEdits(document, options, {
    isCancellationRequested: false,
  } as vscode.CancellationToken);
  Object.assign(document, { version: 2 });
  expect(await pending).toEqual([]);
});

test("sorting runs inside Prettier and leaves ignored nodes unchanged", async () => {
  const requests: unknown[] = [];
  const source = 'const a=style("flex block");\n// prettier-ignore\nconst b=style("flex block");\n';
  const formatted = await formatSource(
    source,
    "typescript",
    {
      insertSpaces: true,
      tabSize: 2,
    },
    {
      filepath: "/workspace/entry.ts",
      async sort(input) {
        requests.push(input.protectedRanges.length);
        const start = input.source.indexOf("flex block");
        const edits = [
          {
            start,
            end: start + 10,
            newText: "block flex",
          },
        ];
        return {
          code: input.source.slice(0, start) + "block flex" + input.source.slice(start + 10),
          edits,
        };
      },
    },
  );
  expect(formatted).toBe(
    'const a = style("block flex");\n// prettier-ignore\nconst b=style("flex block");\n',
  );
  expect(requests).toEqual([1]);
});
