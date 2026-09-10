import type { CompilerMapping } from "./source-map.ts";
import type { AssetRecord, TypeFact } from "./type-facts.ts";
export interface CompilerOutput {
  mappings: CompilerMapping[];
  css: string;
  code: string | null;
  classes: Array<{ class_name: string }>;
  symbols: GeneratedSymbol[];
  diagnostics: Array<{
    code: string;
    message: string;
    file: string;
    start: number;
    end: number;
    start_utf16: number;
    end_utf16: number;
    start_line: number;
    start_column: number;
    end_line: number;
    end_column: number;
    notes: string[];
  }>;
  lint_findings: Array<{
    rule: string;
    message: string;
    file: string;
    start: number;
    end: number;
    notes: string[];
  }>;
  lint_policy: Record<string, "warn" | "deny" | "off">;
}

export interface GeneratedSymbol {
  name: string;
  kind: string;
  identity: string;
  source_path: string;
  start: number;
  end: number;
}

export interface PlayOutput extends CompilerOutput {
  previewUrl: string | null;
}

export interface ModuleSummary {
  module_specifiers: Array<{ source: string; start: number; end: number }>;
  type_probes: Array<{ start: number; end: number }>;
  imports: ImportBinding[];
  re_exports: Array<{ source: string; exported: string; imported: string }>;
  probe_imports: ImportBinding[];
  export_imports: Record<string, ImportBinding[]>;
  assets: AssetRecord[];
}

export interface ImportBinding {
  local: string;
  source: string;
  imported: string;
}

export interface ProjectSource {
  id: string;
  path: string;
  source: string;
  type_facts?: TypeFact[];
}

export interface ProjectCompilerOutput {
  modules: Array<CompilerOutput & { path: string }>;
}
