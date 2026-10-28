import { getService, IExtensionService, initialize } from "@codingame/monaco-vscode-api";
import { registerAssets } from "@codingame/monaco-vscode-api/assets";
import { ExtensionHostKind, registerExtension } from "@codingame/monaco-vscode-api/extensions";
import { createModelReference, writeFile } from "@codingame/monaco-vscode-api/monaco";
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri";
import {
  isIMenuItem,
  MenuId,
  MenuRegistry,
} from "@codingame/monaco-vscode-api/vscode/vs/platform/actions/common/actions";
import "@codingame/monaco-vscode-api/vscode/vs/workbench/contrib/format/browser/formatActionsMultiple";
import EditorWorker from "@codingame/monaco-vscode-api/workers/editor.worker?worker";
import getConfiguration, {
  initUserConfiguration,
} from "@codingame/monaco-vscode-configuration-service-override";
import "@codingame/monaco-vscode-css-default-extension";
import * as monaco from "@codingame/monaco-vscode-editor-api";
import "@codingame/monaco-vscode-editor-api/esm/vs/editor/contrib/format/browser/formatActions";
import getEditor from "@codingame/monaco-vscode-editor-service-override";
import "@codingame/monaco-vscode-extension-api/localExtensionHost";
import getExtensions from "@codingame/monaco-vscode-extensions-service-override";
import getFiles, { initFile } from "@codingame/monaco-vscode-files-service-override";
import "@codingame/monaco-vscode-javascript-default-extension";
import "@codingame/monaco-vscode-json-default-extension";
import getKeybindings from "@codingame/monaco-vscode-keybindings-service-override";
import getLanguages from "@codingame/monaco-vscode-languages-service-override";
import getModels from "@codingame/monaco-vscode-model-service-override";
import getNotifications from "@codingame/monaco-vscode-notifications-service-override";
import getTextmate from "@codingame/monaco-vscode-textmate-service-override";
import TextmateWorker from "@codingame/monaco-vscode-textmate-service-override/worker?worker";
import "@codingame/monaco-vscode-theme-defaults-default-extension";
import getTheme from "@codingame/monaco-vscode-theme-service-override";
import "@codingame/monaco-vscode-typescript-basics-default-extension";
import "@codingame/monaco-vscode-typescript-language-features-default-extension";
import { configurationFiles } from "@stillwind/config";
import type * as vscode from "vscode";

import { formatterConfiguration, formatterManifest, registerFormatter } from "./formatter.ts";
import { normalizeWorkspacePath } from "./path.ts";

/**
 * Editor context menu commands that the playground hides. The workspace contains declarations
 * but no JavaScript sources, so Source Definition finds nothing. The Prettier formatter does not
 * format ranges, so selection formatting and formatter pickers would select the TypeScript
 * formatter and produce a different style from the Format button.
 */
const hiddenEditorCommands = new Set([
  "typescript.goToSourceDefinition",
  "editor.action.formatDocument.multiple",
  "editor.action.formatSelection",
  "editor.action.formatSelection.multiple",
]);

const menuItems = MenuRegistry.getMenuItems.bind(MenuRegistry);
MenuRegistry.getMenuItems = (id) => {
  const items = menuItems(id);
  return id === MenuId.EditorContext
    ? items.filter((item) => !(isIMenuItem(item) && hiddenEditorCommands.has(item.command.id)))
    : items;
};

/**
 * VS Code services, editor buffers, and dependency files used by one playground session.
 */
export class PlaygroundWorkspace {
  /** API for commands, workspace edits, and diagnostics in the initialized extension host. */
  readonly api: typeof vscode;
  /** Editable original files keyed by absolute virtual path. */
  readonly files: Map<string, string>;
  readonly #references = new Map<string, Awaited<ReturnType<typeof createModelReference>>>();
  #editor?: monaco.editor.IStandaloneCodeEditor;
  #changed: () => void = () => {};
  #formatter?: vscode.Disposable;

  private constructor(api: typeof vscode, files: Map<string, string>) {
    this.api = api;
    this.files = files;
  }

  /**
   * Loads dependencies and activates the public Web extension in VS Code services.
   *
   * @param initial - Editable workspace files keyed by absolute virtual path. They must include
   *   `/workspace/App.tsx`.
   * @returns The page's workspace with open source buffers and active language clients.
   * @throws If isolation headers, static assets, or extension activation are unavailable.
   */
  static async open(initial: ReadonlyMap<string, string>): Promise<PlaygroundWorkspace> {
    if (!crossOriginIsolated || typeof SharedArrayBuffer === "undefined") {
      throw new Error(
        "Serve this site with COOP: same-origin and COEP: require-corp to run the Stillwind compiler.",
      );
    }
    const base = new URL(import.meta.env.BASE_URL, location.href);
    const response = await fetch(new URL("workspace.json", base));
    if (!response.ok) {
      throw new Error("Cannot load the playground dependency inventory");
    }
    const packages = (await response.json()) as [string, string][];
    for (const [file, text] of [...packages, ...initial]) {
      await initFile(URI.file(file), text);
    }
    await initUserConfiguration(
      JSON.stringify({
        "workbench.colorTheme": "Default Dark Modern",
        "workbench.colorCustomizations": themeColors(),
        "editor.minimap.enabled": false,
        ...formatterConfiguration,
        "js/ts.disableAutomaticTypeAcquisition": true,
        "js/ts.tsserver.web.projectWideIntellisense.enabled": true,
        "js/ts.tsserver.web.typeAcquisition.enabled": false,
        "js/ts.tsserver.web.projectWideIntellisense.suppressSemanticErrors": false,
        "js/ts.experimental.useTsgo": false,
        "typescript.experimental.useTsgo": false,
      }),
    );
    const embedding = registerExtension(
      {
        name: "playground",
        publisher: "stillwind-internal",
        version: "0.0.0",
        engines: { vscode: "*" },
      },
      ExtensionHostKind.LocalProcess,
    );
    const formatter = registerExtension(formatterManifest, ExtensionHostKind.LocalProcess);
    const manifest = await fetch(new URL("extensions/stillwind/package.json", base)).then(
      (result) => result.json(),
    );
    const extension = registerExtension(manifest, ExtensionHostKind.LocalWebWorker);
    for (const file of [
      "extension.js",
      "server.js",
      "server-module.js",
      "libraries.json",
      "esbuild.wasm",
      "stillwind_native.wasm32-wasi.wasm",
      "@stillwind/native-wasm32-wasi/wasi-worker-browser.mjs",
    ]) {
      extension.registerFileUrl(
        `dist/browser/${file}`,
        new URL(`extensions/stillwind/dist/browser/${file}`, base).href,
      );
    }
    const extensionWorker = new URL("extension-host.js", base).href;
    registerAssets({ "vs/workbench/api/worker/extensionHostWorkerMain.js": extensionWorker });
    Object.assign(window, {
      MonacoEnvironment: {
        getWorkerUrl: (_id: string, label: string) =>
          label === "extensionHostWorkerMain" ? extensionWorker : undefined,
        getWorker: (_id: string, label: string) =>
          label === "TextMateWorker"
            ? new TextmateWorker()
            : label === "editorWorkerService"
              ? new EditorWorker()
              : undefined,
      },
    });
    await initialize(
      {
        ...getFiles(),
        ...getConfiguration(),
        ...getModels(),
        ...getLanguages(),
        ...getKeybindings(),
        ...getNotifications(),
        ...getEditor(async () => undefined),
        ...getExtensions({
          enableWorkerExtensionHost: true,
        }),
        ...getTextmate(),
        ...getTheme(),
      },
      undefined,
      {
        workspaceProvider: {
          workspace: {
            folderUri: URI.file("/workspace"),
          },
          trusted: true,
          open: async () => false,
        },
        developmentOptions: { logLevel: 2 },
      },
    );
    const api = await embedding.getApi();
    await embedding.setAsDefaultApi();
    const workspace = new PlaygroundWorkspace(api, new Map(initial));
    workspace.#formatter = registerFormatter(await formatter.getApi());
    for (const file of workspace.files.keys()) {
      await workspace.#reference(file);
      await api.workspace.openTextDocument(api.Uri.file(file));
    }
    await (await getService(IExtensionService)).activateByEvent("onLanguage:typescriptreact");
    await extension.whenReady();
    await (await getService(IExtensionService)).activateByEvent("onCommand:stillwind.snapshot");
    await api.commands.executeCommand("stillwind.snapshot");
    return workspace;
  }

  /**
   * Creates a Monaco editor connected to the extension host's buffers.
   *
   * @param element - Container whose size Monaco observes.
   * @param changed - Called after an editable buffer changes or a file is added.
   */
  mount(element: HTMLElement, changed: () => void): void {
    this.#changed = changed;
    this.#editor = monaco.editor.create(element, {
      ...editorAppearance(),
      wordWrap: "on",
      editContext: false,
    });
    this.select("/workspace/App.tsx");
  }

  /**
   * Runs the selected buffer's default formatter. Prettier sorts Stillwind utilities through
   * `prettier-plugin-stillwind`.
   */
  async format(): Promise<void> {
    await this.#editor?.getAction("editor.action.formatDocument")?.run();
  }

  /**
   * Lists Stillwind fix-all actions for one buffer.
   *
   * @param file - Absolute virtual path of an open buffer.
   * @returns Enabled actions with edits, one for each project whose lint findings offer fixes.
   */
  async fixes(file: string): Promise<vscode.CodeAction[]> {
    const document = await this.api.workspace.openTextDocument(this.api.Uri.file(file));
    const actions = await this.api.commands.executeCommand<(vscode.CodeAction | vscode.Command)[]>(
      "vscode.executeCodeActionProvider",
      document.uri,
      new this.api.Range(document.positionAt(0), document.positionAt(document.getText().length)),
      "source.fixAll.stillwind",
    );
    return (actions ?? []).filter(
      (action): action is vscode.CodeAction =>
        "kind" in action &&
        action.kind?.value === "source.fixAll.stillwind" &&
        !action.disabled &&
        action.edit !== undefined,
    );
  }

  /**
   * Applies Stillwind lint fixes to one buffer only.
   *
   * @param file - Absolute virtual path of an open buffer.
   * @throws If projects offer different fixes or the fix edit cannot be applied.
   */
  async fix(file: string): Promise<void> {
    const fixes = await this.fixes(file);
    if (fixes.length > 1) {
      throw new Error("Multiple Stillwind projects offer different fixes for this file.");
    }
    if (fixes[0] && !(await this.api.workspace.applyEdit(fixes[0].edit!))) {
      throw new Error("Cannot apply the Stillwind fix edit. Try again.");
    }
  }

  /**
   * Shows an existing source buffer; unknown files are ignored.
   *
   * @param file - Absolute virtual path in the editable file map.
   * @param range - Optional zero-based UTF-16 diagnostic range to select and reveal.
   */
  select(file: string, range?: vscode.Range): void {
    const model = this.#references.get(file)?.object.textEditorModel;
    if (!model || !this.#editor) {
      return;
    }
    this.#editor.setModel(model);
    if (range) {
      const selection = new monaco.Range(
        range.start.line + 1,
        range.start.character + 1,
        range.end.line + 1,
        range.end.character + 1,
      );
      this.#editor.setSelection(selection);
      this.#editor.revealRangeInCenter(selection);
      this.#editor.focus();
    }
  }

  /**
   * Adds an empty source file and opens its buffer.
   *
   * @param name - JS, TS, TSX, JSX, CSS, or JSON path relative to the workspace. It is normalized
   *   with `normalizeWorkspacePath`.
   * @returns Absolute virtual path of the new file.
   * @throws If the normalized path is `null`, has another extension, or already exists. No file
   *   is created in that case.
   */
  async add(name: string): Promise<string> {
    const path = normalizeWorkspacePath(name);
    if (!path || !/\.(?:[cm]?[jt]s|tsx|jsx|css|json)$/.test(path)) {
      throw new Error("Use a relative JS, TS, TSX, CSS, or JSON filename inside the workspace");
    }
    const file = `/workspace/${path}`;
    if (this.files.has(file)) {
      throw new Error("This file already exists");
    }
    await writeFile(URI.file(file), "");
    this.files.set(file, "");
    await this.#reference(file);
    this.select(file);
    this.#changed();
    return file;
  }

  /**
   * Saves dirty configuration and theme buffers before evaluation.
   *
   * @returns Whether any saved settings require workspace configuration to be reloaded.
   */
  async saveSettings(): Promise<boolean> {
    let reload = false;
    for (const [file, reference] of this.#references) {
      if (
        reference.object.isDirty() &&
        (file.endsWith(".css") ||
          file.endsWith(".json") ||
          configurationFiles.some((name) => file.endsWith(`/${name}`)))
      ) {
        await reference.object.save();
        reload = true;
      }
    }
    return reload;
  }

  /**
   * Releases editor references and model listeners.
   */
  dispose(): void {
    this.#editor?.dispose();
    this.#formatter?.dispose();
    for (const reference of this.#references.values()) {
      reference.dispose();
    }
  }

  async #reference(file: string): Promise<void> {
    const reference = await createModelReference(URI.file(file));
    this.#references.set(file, reference);
    reference.object.textEditorModel?.onDidChangeContent(() => {
      this.files.set(file, reference.object.textEditorModel!.getValue());
      this.#changed();
    });
  }
}

/**
 * Returns the editor options that the source editor and the output view share.
 */
export function editorAppearance(): monaco.editor.IStandaloneEditorConstructionOptions {
  return {
    automaticLayout: true,
    fontSize: 13,
    lineHeight: 20,
    fontFamily: token("--font-mono"),
    padding: { top: 8 },
    scrollBeyondLastLine: false,
    minimap: {
      enabled: false,
    },
  };
}

/**
 * Reads a theme token from the playground stylesheet on the document root.
 */
function token(name: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

/**
 * Maps playground theme tokens to VS Code color identifiers for the editor and its widgets.
 */
function themeColors(): Record<string, string> {
  const editor = token("--color-editor");
  const widget = token("--color-editor-widget");
  const border = token("--color-border");
  return {
    focusBorder: token("--color-accent"),
    "editor.background": editor,
    "editorGutter.background": editor,
    "editor.lineHighlightBackground": token("--color-editor-line"),
    "editor.lineHighlightBorder": token("--color-editor-line"),
    "editor.selectionBackground": token("--color-editor-selection"),
    "editorLineNumber.foreground": token("--color-editor-gutter"),
    "editorLineNumber.activeForeground": token("--color-muted"),
    "editorCursor.foreground": token("--color-accent"),
    "editorWidget.background": widget,
    "editorWidget.border": border,
    "editorHoverWidget.background": widget,
    "editorHoverWidget.border": border,
    "editorSuggestWidget.background": widget,
    "editorSuggestWidget.border": border,
    "editorSuggestWidget.selectedBackground": token("--color-selection"),
    "quickInput.background": widget,
    "list.hoverBackground": token("--color-surface-muted"),
    "list.activeSelectionBackground": token("--color-selection"),
  };
}
