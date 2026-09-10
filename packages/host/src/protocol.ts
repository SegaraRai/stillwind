import type { BindingOutput } from "@stillwind/native";
import type { ProjectSource as FrameworkSource } from "@stillwind/projection";

import type { TypeFact } from "./type-facts.ts";

export type {
  BindingOutput as CompilerOutput,
  NativeGeneratedSymbol as GeneratedSymbol,
  NativeModuleSummary as ModuleSummary,
  NativeImportBinding as ImportBinding,
  ProjectBindingOutput as ProjectCompilerOutput,
} from "@stillwind/native";

export interface PlayOutput extends BindingOutput {
  previewUrl: string | null;
}

export interface ProjectSource extends FrameworkSource {
  type_facts?: TypeFact[];
}

export type { ProjectionMap } from "@stillwind/projection";
