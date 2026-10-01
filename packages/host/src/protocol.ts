import type { PreparedSource as ProjectionPreparedSource } from "@stillwind/projection";

import type { TypeFact } from "./type-facts.ts";

export type {
  ProjectInput,
  ProjectionMap,
  ScriptSyntax,
  SourceDocument,
  SourceProjection,
} from "@stillwind/projection";

export type {
  BindingOutput as CompilerOutput,
  NativeGeneratedSymbol as GeneratedSymbol,
  NativeModuleSummary as ModuleSummary,
  NativeImportBinding as ImportBinding,
  ProjectBindingOutput as ProjectCompilerOutput,
} from "@stillwind/native";

export type PreparedSource = ProjectionPreparedSource & {
  typeFacts?: TypeFact[];
};
