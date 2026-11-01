import type {
  NativeCssFileInput,
  NativeCssLoadStep,
  NativeCssRequest,
  NativeDesignSystemLoader,
} from "@stillwind/native";

/**
 * One CSS source that a design-system loader receives, without the request id.
 */
export type StylesheetSource = Omit<NativeCssFileInput, "id">;

/**
 * Sources of a recorded CSS load. A loader that replays them builds the same design system
 * without resolving or reading files.
 */
export interface RecordedStylesheet {
  /** The root source, which the loader receives as id `0`. */
  root: StylesheetSource;
  /** Answers keyed by `importKey()` of the importing source name and the import specifier. */
  imports: Record<string, StylesheetSource>;
}

/**
 * Returns the key of `imports` in a recorded stylesheet for one import request.
 */
export function importKey(request: Pick<NativeCssRequest, "importer" | "specifier">): string {
  return `${request.importer}\n${request.specifier}`;
}

/**
 * Feeds `recorded` to `loader`, which must not have received any CSS input, and returns the
 * completed load step.
 *
 * @throws If the loader requests an import that the recording does not contain, or a plugin.
 */
export function replayStylesheet(
  loader: NativeDesignSystemLoader,
  recorded: RecordedStylesheet,
): Extract<NativeCssLoadStep, { kind: "complete" }> {
  const { root } = recorded;
  let step = loader.provide(0, root.name, root.content, root.uri, root.sourceMapSource, root.lint);
  const answered = new Set<number>();
  while (step.kind === "needFile") {
    // A step repeats every outstanding request, so each id is answered once.
    const files = step.requests
      .filter((request) => !answered.has(request.id))
      .map((request) => {
        const source = recorded.imports[importKey(request)];
        if (request.pluginOptions !== undefined || !source) {
          throw new Error(`The recorded stylesheet cannot answer ${request.specifier}`);
        }
        answered.add(request.id);
        return {
          id: request.id,
          ...source,
        };
      });
    if (files.length === 0) {
      throw new Error("The loader did not request a new stylesheet");
    }
    step = loader.provideMany(files);
  }
  return step;
}
