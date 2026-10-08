import { getService, IExtensionService, initialize } from "@codingame/monaco-vscode-api";
import { registerAssets } from "@codingame/monaco-vscode-api/assets";
import { ExtensionHostKind, registerExtension } from "@codingame/monaco-vscode-api/extensions";
import { createModelReference, writeFile } from "@codingame/monaco-vscode-api/monaco";
import { URI } from "@codingame/monaco-vscode-api/vscode/vs/base/common/uri";
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

import { exampleFiles } from "./example.ts";
import { formatterConfiguration, formatterManifest, registerFormatter } from "./formatter.ts";

/** VS Code services, editor buffers, and dependency files used by one playground session. */
export class PlaygroundWorkspace {
  /** API for commands, workspace edits, and diagnostics in the initialized extension host. */
  readonly api: typeof vscode;
  /** Editable original files keyed by absolute virtual path. */
  readonly files = new Map(exampleFiles);
  readonly #references = new Map<string, Awaited<ReturnType<typeof createModelReference>>>();
  #editor?: monaco.editor.IStandaloneCodeEditor;
  #changed: () => void = () => {};
  #formatter?: vscode.Disposable;

  private constructor(api: typeof vscode) {
    this.api = api;
  }

  /**
   * Loads dependencies and activates the public Web extension in VS Code services.
   * @returns The page's workspace with open source buffers and active language clients.
   * @throws If isolation headers, static assets, or extension activation are unavailable.
   */
  static async open(): Promise<PlaygroundWorkspace> {
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
    for (const [file, text] of [...packages, ...exampleFiles]) {
      await initFile(URI.file(file), text);
    }
    await initUserConfiguration(
      JSON.stringify({
        "workbench.colorTheme": "Default Dark Modern",
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
    const workspace = new PlaygroundWorkspace(api);
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
   * @param element - Container whose size Monaco observes.
   * @param changed - Called after an editable buffer changes or a file is added.
   */
  mount(element: HTMLElement, changed: () => void): void {
    this.#changed = changed;
    this.#editor = monaco.editor.create(element, {
      automaticLayout: true,
      fontSize: 13,
      fontFamily: "Consolas, monospace",
      minimap: {
        enabled: false,
      },
      wordWrap: "on",
      editContext: false,
    });
    this.select("/workspace/Component.tsx");
  }

  /**
   * Applies Stillwind sorting to the selected buffer, then runs its default formatter.
   * Skips changed buffers and ignores unavailable or disabled sort actions.
   * @throws If projects offer conflicting sort actions or the sort edit cannot be applied.
   */
  async format(): Promise<void> {
    const editor = this.#editor;
    const model = editor?.getModel();
    if (!editor || !model) {
      return;
    }
    const version = model.getVersionId();
    const document = await this.api.workspace.openTextDocument(
      this.api.Uri.parse(model.uri.toString()),
    );
    const actions = await this.api.commands.executeCommand<(vscode.CodeAction | vscode.Command)[]>(
      "vscode.executeCodeActionProvider",
      document.uri,
      new this.api.Range(document.positionAt(0), document.positionAt(document.getText().length)),
      "source.sort.stillwind",
    );
    if (editor.getModel() !== model || model.getVersionId() !== version) {
      return;
    }
    const sorts = (actions ?? []).filter(
      (action): action is vscode.CodeAction =>
        "kind" in action && action.kind?.value === "source.sort.stillwind" && !action.disabled,
    );
    if (sorts.length > 1) {
      throw new Error(
        "Multiple Stillwind projects offer different sorts. Choose a sort Code Action first.",
      );
    }
    const sort = sorts[0];
    if (sort?.edit && !(await this.api.workspace.applyEdit(sort.edit))) {
      throw new Error("Cannot apply the Stillwind sort edit. Try formatting again.");
    }
    if (sort?.command) {
      await this.api.commands.executeCommand(
        sort.command.command,
        ...(sort.command.arguments ?? []),
      );
    }
    if (editor.getModel() === model) {
      await editor.getAction("editor.action.formatDocument")?.run();
    }
  }

  /**
   * Shows an existing source buffer; unknown files are ignored.
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
   * @param name - Relative JS, TS, TSX, JSX, CSS, or JSON path without parent directory segments.
   * @throws If the path is invalid or already exists.
   */
  async add(name: string): Promise<void> {
    if (
      !/^[\w./-]+\.(?:[cm]?[jt]s|tsx|jsx|css|json)$/.test(name) ||
      name.split("/").includes("..")
    ) {
      throw new Error("Use a relative JS, TS, TSX, CSS, or JSON filename inside the workspace");
    }
    const file = `/workspace/${name.replace(/^\/+/, "")}`;
    if (this.files.has(file)) {
      throw new Error("This file already exists");
    }
    await writeFile(URI.file(file), "");
    this.files.set(file, "");
    await this.#reference(file);
    this.select(file);
    this.#changed();
  }

  /**
   * Saves dirty configuration and theme buffers before evaluation.
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

  /** Releases editor references and model listeners. */
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
