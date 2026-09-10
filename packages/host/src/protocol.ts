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
  type_probes: Array<{ start: number; end: number }>;
  imports: Array<{ source: string }>;
  re_exports: Array<{ source: string }>;
  assets: AssetRecord[];
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
