/**
 * Prepares original documents for compiler parsing and TypeScript checks.
 * `prepareSource` loads the converter selected by the document format. `checkerEnvironment`
 * locates required declarations and separates Astro client scripts from the main checker buffer.
 * Projection regions use original UTF-16 offsets, while maps describe generated-to-original positions.
 */

export { checkerEnvironment, type CheckerBuffer, type CheckerEnvironment } from "./checker.ts";
export { prepareSource } from "./frameworks/index.ts";
export { PreparationError } from "./preparation.ts";
export type {
  ConverterIdentity,
  PreparedSource,
  ProjectInput,
  ProjectionMap,
  ProjectMetadata,
  ScriptSyntax,
  SourceDocument,
  SourceFormat,
  SourceProjection,
} from "./protocol.ts";
