import { compileModule, TypeScriptFactsService, type CompilerHost } from "@stillwind/host";
import { NativeProjectSession, type NativeDesignSystemLoader } from "@stillwind/native";

import type { StillwindOptions } from "./options.ts";

/** Owns compiler and lazy type facts for one adapter environment or reconstruction. */
export class IntegrationSession {
  readonly native: NativeProjectSession;
  readonly root: string;
  private facts?: TypeScriptFactsService;

  constructor(
    root: string,
    loader: NativeDesignSystemLoader,
    options: StillwindOptions,
    watch: boolean,
    mode: string,
  ) {
    this.root = root;
    this.native = new NativeProjectSession(loader, watch, mode, options.naming);
  }

  compile(
    code: string,
    id: string,
    host: CompilerHost,
    transport: Parameters<typeof compileModule>[6] = {},
  ) {
    return compileModule(
      this.native,
      () => (this.facts ??= new TypeScriptFactsService(this.root)),
      code,
      id,
      this.root,
      host,
      transport,
    );
  }

  close(): void {
    this.facts?.close();
    this.facts = undefined;
  }
}
