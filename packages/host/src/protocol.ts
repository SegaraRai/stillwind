import type { ProjectSource as FrameworkSource } from "@stillwind/projection";

import type { TypeFact } from "./type-facts.ts";

export type { ProjectionMap } from "@stillwind/projection";

export type {
  BindingOutput as CompilerOutput,
  NativeGeneratedSymbol as GeneratedSymbol,
  NativeModuleSummary as ModuleSummary,
  NativeImportBinding as ImportBinding,
  ProjectBindingOutput as ProjectCompilerOutput,
} from "@stillwind/native";

export interface ProjectSource extends FrameworkSource {
  type_facts?: TypeFact[];
}
