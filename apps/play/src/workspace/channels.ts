import { getService, IOutputService, ITextModelService } from "@codingame/monaco-vscode-api";
import { Registry } from "@codingame/monaco-vscode-api/vscode/vs/platform/registry/common/platform";
import {
  Extensions,
  type IOutputChannelRegistry,
} from "@codingame/monaco-vscode-api/vscode/vs/workbench/services/output/common/output";
import * as monaco from "@codingame/monaco-vscode-editor-api";

import { editorAppearance } from "./editor.ts";

/**
 * Output channel that the panel can show.
 */
export interface ChannelEntry {
  /** Channel identifier that `OutputChannels.show` accepts. */
  id: string;
  /** Name that the extension or the workbench gave the channel. */
  label: string;
}

/**
 * Shows the output channels of VS Code services and extensions in a read-only Monaco editor,
 * like the Output panel of VS Code. The view follows new output while it is scrolled to the end.
 * Create the view after `PlaygroundWorkspace.open()` has initialized the VS Code services.
 *
 * The playground has no Output view, so the view reports each request to show a channel, such as
 * `OutputChannel.show()` in an extension or the "Go to output" button of a notification.
 */
export class OutputChannels {
  readonly #editor: monaco.editor.IStandaloneCodeEditor;
  readonly #registry = Registry.as<IOutputChannelRegistry>(Extensions.OutputChannels);
  readonly #listeners: monaco.IDisposable[] = [];
  #reference?: Awaited<ReturnType<ITextModelService["createModelReference"]>>;
  #content?: monaco.IDisposable;
  /** Increments for each `show` call, so a slower earlier call does not replace a later one. */
  #request = 0;
  /** Whether the end of the channel is visible, so appended output scrolls into view. */
  #following = true;
  #disposed = false;

  /**
   * @param element - Container whose size Monaco observes.
   * @param changed - Called after a channel is added or removed.
   * @param requested - Called with the channel identifier of each request to show a channel. The
   *   request does not change the channel that this view shows.
   */
  constructor(element: HTMLElement, changed: () => void, requested: (id: string) => void) {
    this.#editor = monaco.editor.create(element, {
      ...editorAppearance(),
      readOnly: true,
      domReadOnly: true,
      lineNumbers: "off",
      folding: false,
      wordWrap: "on",
      renderValidationDecorations: "off",
      editContext: false,
    });
    this.#listeners.push(
      this.#registry.onDidRegisterChannel(changed),
      this.#registry.onDidRemoveChannel(changed),
      this.#editor.onDidScrollChange((event) => {
        this.#following =
          event.scrollTop + this.#editor.getLayoutInfo().height >= event.scrollHeight - 1;
      }),
    );
    void getService(IOutputService).then((outputs) => {
      if (this.#disposed) {
        return;
      }
      const show = outputs.showChannel.bind(outputs);
      outputs.showChannel = async (id, preserveFocus) => {
        requested(id);
        await show(id, preserveFocus);
      };
      this.#listeners.push({
        // Removing the own property exposes the method of the service class again.
        dispose: () => Reflect.deleteProperty(outputs, "showChannel"),
      });
    });
  }

  /**
   * Lists the registered channels in registration order.
   */
  list(): ChannelEntry[] {
    return this.#registry.getChannels().map(({ id, label }) => ({ id, label }));
  }

  /**
   * Shows one channel and scrolls to its end. Unknown identifiers leave the view unchanged.
   *
   * @param id - Identifier from `list`.
   */
  async show(id: string): Promise<void> {
    const request = ++this.#request;
    const [outputs, models] = await Promise.all([
      getService(IOutputService),
      getService(ITextModelService),
    ]);
    const channel = outputs.getChannel(id);
    if (!channel) {
      return;
    }
    const reference = await models.createModelReference(channel.uri);
    if (request !== this.#request) {
      reference.dispose();
      return;
    }
    this.#content?.dispose();
    this.#reference?.dispose();
    this.#reference = reference;
    const model = reference.object.textEditorModel as unknown as monaco.editor.ITextModel;
    this.#editor.setModel(model);
    this.#following = true;
    this.#reveal(model);
    this.#content = model.onDidChangeContent(() => {
      if (this.#following) {
        this.#reveal(model);
      }
    });
  }

  /**
   * Releases the editor, the shown channel model, and the registry listeners.
   */
  dispose(): void {
    this.#disposed = true;
    for (const listener of this.#listeners) {
      listener.dispose();
    }
    this.#content?.dispose();
    this.#reference?.dispose();
    this.#editor.dispose();
  }

  #reveal(model: monaco.editor.ITextModel): void {
    this.#editor.revealLine(model.getLineCount());
  }
}
