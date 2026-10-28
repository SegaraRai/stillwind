import { getService, ILanguageService, IModelService } from "@codingame/monaco-vscode-api";
import * as monaco from "@codingame/monaco-vscode-editor-api";

import { editorAppearance } from "./editor.ts";

/**
 * Shows build outputs in a read-only Monaco editor with TextMate highlighting.
 * Output models are created for a simple widget, so VS Code does not send them to extensions.
 * Language servers therefore neither analyze nor report diagnostics for them.
 * Create the view after `PlaygroundWorkspace.open()` has initialized the VS Code services.
 */
export class OutputView {
  readonly #editor: monaco.editor.IStandaloneCodeEditor;

  /** Output models keyed by the identifier passed to `show`. */
  readonly #models = new Map<string, monaco.editor.ITextModel>();

  /** @param element - Container whose size Monaco observes. */
  constructor(element: HTMLElement) {
    this.#editor = monaco.editor.create(element, {
      ...editorAppearance(),
      readOnly: true,
      domReadOnly: true,
      renderValidationDecorations: "off",
      editContext: false,
    });
  }

  /**
   * Shows one output. Each identifier keeps its own model, so switching back keeps its scroll
   * position until its text changes.
   *
   * @param id - Stable identifier such as `css:/workspace/App.tsx`.
   * @param language - VS Code language ID used for highlighting.
   * @param text - Complete output text.
   */
  async show(id: string, language: string, text: string): Promise<void> {
    let model = this.#models.get(id);
    if (!model) {
      const [models, languages] = await Promise.all([
        getService(IModelService),
        getService(ILanguageService),
      ]);
      model = this.#models.get(id);
      if (!model) {
        model = models.createModel(
          text,
          languages.createById(language),
          monaco.Uri.from({ scheme: "stillwind-output", path: `/${id}` }),
          true,
        ) as unknown as monaco.editor.ITextModel;
        this.#models.set(id, model);
      }
    }
    if (model.getValue() !== text) {
      model.setValue(text);
    }
    if (model.getLanguageId() !== language) {
      monaco.editor.setModelLanguage(model, language);
    }
    if (this.#editor.getModel() !== model) {
      this.#editor.setModel(model);
    }
  }

  /**
   * Releases the editor and its output models.
   */
  dispose(): void {
    this.#editor.dispose();
    for (const model of this.#models.values()) {
      model.dispose();
    }
  }
}
