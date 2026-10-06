import { cleanModuleId, compilerModuleId } from "@stillwind/host";

import type { BrowserCssSources } from "./browser-css.ts";
import type { BuildOutput } from "./build.ts";
import { compileProducer, type CompileContext, type ProducerInput } from "./compile.ts";
import { DesignSystem, type CssContext, type DesignSystemStatus } from "./design-system.ts";
import { ModuleRegistry } from "./module-registry.ts";
import type { IntegrationConfig, ProjectSettings } from "./project.ts";
import { resolveStylesheet, type StylesheetPlan } from "./stylesheet.ts";

/** A development stylesheet snapshot and producer dependency graph for one environment. */
export interface CssSnapshot {
  /** Environment revision used by the runtime to discard older stylesheet snapshots. */
  revision: number;
  /** Stylesheet occurrences in cascade order, with resource URLs and optional browser-safe maps. */
  parts: StylesheetPlan["parts"];
  /**
   * Compiler module identities mapped to their composition dependencies, including manual
   * producers.
   */
  dependencies: Record<string, string[]>;
}

/** Owns compiler inputs, virtual references, and serialized CSS work for one Vite environment. */
export class ViteEnvironment {
  /** Design system and compiler used only by this environment. */
  readonly #designSystem: DesignSystem;
  /** Producer inputs keyed by their full Vite identities, including query suffixes. */
  readonly #inputs = new Map<string, ProducerInput>();
  /**
   * Compiler module identities whose JavaScript changed after a CSS reload, pending consumption by
   * hot-update handling.
   */
  readonly #themeChanges = new Set<string>();

  /**
   * Promise released by the latest queued operation. It resolves even when that operation fails, so
   * later work can continue.
   */
  #work = Promise.resolve();
  /** Virtual references created lazily after the project root has been configured. */
  #registry?: ModuleRegistry;

  /** Revision counter advanced when the compiler initializes or a snapshot is published. */
  revision = 0;
  /**
   * Production assets and manifest from the latest stylesheet emission. Build start clears this
   * value.
   */
  buildOutput?: BuildOutput;

  /**
   * Creates isolated compilation state for a Vite environment.
   *
   * @param name - Vite environment name used for virtual module identities and runtime snapshots.
   * @param settings - Shared project settings that must be configured before compilation starts.
   * @param config - Compiler options shared by the project.
   * @param invalidate - Invalidates Vite source identities whose JavaScript changes after a
   * design-system reload.
   */
  constructor(
    readonly name: string,
    private readonly settings: ProjectSettings,
    private readonly config: IntegrationConfig,
    private readonly invalidate: (ids: readonly string[]) => void,
  ) {
    this.#designSystem = new DesignSystem(settings, config);
  }

  /**
   * Exposes design-system reload progress without allowing callers to replace its state.
   *
   * @returns The current environment's live design-system status.
   */
  get designSystem(): DesignSystemStatus {
    return this.#designSystem;
  }

  /**
   * Exposes compiled producers for stylesheet selection and graph recovery.
   *
   * @returns The live producer map keyed by full Vite source identity. Callers must not mutate its
   * entries.
   */
  get inputs(): ReadonlyMap<string, Readonly<ProducerInput>> {
    return this.#inputs;
  }

  /**
   * Obtains the registry of virtual references for this environment.
   *
   * @returns The existing registry, or a registry created using the configured project root.
   */
  get registry(): ModuleRegistry {
    this.#registry ??= new ModuleRegistry(this.settings.projectRoot, this.name);
    return this.#registry;
  }

  /**
   * Collects source metadata for composing browser-visible CSS maps.
   *
   * @returns The project root, environment name, current CSS file set, and current producer inputs.
   */
  get browserSources(): BrowserCssSources {
    return {
      root: this.settings.projectRoot,
      environment: this.name,
      cssFiles: this.#designSystem.cssFiles,
      inputs: [...this.#inputs.values()],
    };
  }

  /**
   * Records a changed file for the next design-system refresh.
   *
   * @param file - Absolute path of the changed file.
   */
  changed(file: string): void {
    this.#designSystem.changed(file);
  }

  /**
   * Closes the environment's type-facts service while retaining native compiler state and producer
   * inputs.
   */
  closeTypeFacts(): void {
    this.#designSystem.close();
  }

  /**
   * Closes type facts and clears virtual references. Producer inputs and native compiler state
   * remain allocated.
   */
  close(): void {
    this.closeTypeFacts();
    this.#registry?.clear();
  }

  /**
   * Starts a compilation cycle by clearing inputs and loading a fresh design system.
   *
   * The operation clears virtual references and build output. It runs after earlier environment
   * operations have finished.
   *
   * @param context - Vite resolver and watch operations for this environment.
   * @returns A promise that resolves after compiler initialization, or rejects if initialization
   * fails.
   */
  start(context: CssContext) {
    return this.#inTurn(async () => {
      this.#designSystem.reset();
      this.#inputs.clear();
      this.registry.clear();
      this.buildOutput = undefined;
      await this.#designSystem.ensure(context, () => {
        this.revision++;
      });
    });
  }

  /**
   * Applies any pending CSS changes and records producers whose JavaScript changed.
   *
   * @param context - Vite resolver and watch operations.
   * @returns A promise that resolves after the serialized refresh. Initial loading failures reject
   * it.
   */
  refreshDesignSystem(context: CssContext) {
    return this.#inTurn(async () => {
      await this.#refresh(context);
    });
  }

  /**
   * Compiles a source producer and records its input for document or explicit CSS emission.
   *
   * Compilation runs after design-system refresh and earlier environment work. A successful result
   * replaces the recorded producer input.
   *
   * @param code - Source text received before Stillwind transformation.
   * @param id - Full Vite source identity, including any query suffix.
   * @param context - Vite resolution, watch, warning, and optional upstream-map operations.
   * @returns A promise for the transformed module, or `undefined` when the compiler produces no
   * code.
   */
  compile(code: string, id: string, context: CompileContext) {
    return this.#compile(code, id, context, false);
  }

  /**
   * Compiles source for a stylesheet projection and forces its styles into document selection.
   *
   * A successful input is marked as a projection and made automatic even when the source uses
   * explicit CSS values.
   *
   * @param code - Original producer source read for the projection.
   * @param id - Full identity of the projected producer.
   * @param context - Resolver, watcher, and warning operations for the projection.
   * @returns A promise for the transformed producer, or `undefined` when the compiler produces no
   * code.
   */
  compileProjection(code: string, id: string, context: CompileContext) {
    return this.#compile(code, id, context, true);
  }

  /**
   * Serializes producer compilation and updates recorded input after a successful transform.
   *
   * @param code - Producer source before Stillwind compilation.
   * @param id - Full Vite producer identity.
   * @param context - Compilation operations and optional upstream source map.
   * @param projection - Whether to mark the result as an automatic stylesheet projection.
   * @returns A promise for the transformed module, or `undefined` if no code was produced.
   */
  #compile(code: string, id: string, context: CompileContext, projection: boolean) {
    return this.#inTurn(async () => {
      await this.#refresh(context);
      const output = await compileProducer(
        this.#designSystem.compiler,
        this.settings,
        this.config,
        this.registry,
        code,
        id,
        context,
        this.#inputs.get(id),
      );
      if (!output) {
        return;
      }
      if (projection) {
        output.input.manual = false;
        output.input.projection = true;
      }
      this.#inputs.set(id, output.input);
      return output.result;
    });
  }

  /**
   * Marks a recovered producer as a stylesheet projection.
   *
   * @param id - Exact identity of an input already recorded in this environment.
   * @returns A promise that resolves after the input is updated. Missing inputs have no effect.
   */
  markProjection(id: string) {
    return this.#inTurn(async () => {
      const input = this.#inputs.get(id);
      if (input) {
        this.#inputs.set(id, { ...input, projection: true });
      }
    });
  }

  /**
   * Replaces the stylesheet projection imports recorded for an importer.
   *
   * @param id - Full identity of the importing module.
   * @param sources - Projection specifiers mapped to their resolved producer identities.
   * @returns A promise that resolves after registry references are updated and unused previous
   * references are removed.
   */
  projections(id: string, sources: ReadonlyMap<string, string>) {
    return this.#inTurn(async () => {
      this.registry.projections(id, sources);
    });
  }

  /**
   * Removes one source identity from producer inputs, virtual references, and the native compiler.
   *
   * @param id - Exact producer identity, including its query suffix.
   * @returns A promise resolving to true when a producer input was removed.
   */
  removeSource(id: string) {
    return this.#remove(id, false);
  }

  /**
   * Removes every producer identity associated with a file, including query-qualified inputs.
   *
   * @param id - File identity. Its query is ignored when matching producer inputs and references.
   * @returns A promise resolving to true when at least one producer input was removed.
   */
  removeFile(id: string) {
    return this.#remove(id, true);
  }

  /**
   * Removes matching producer inputs and their associated compiler artifacts.
   *
   * @param id - Producer or file identity to remove.
   * @param file - Whether to match all query variants of the underlying file.
   * @returns A promise resolving to true when at least one recorded input was removed.
   */
  #remove(id: string, file: boolean) {
    return this.#inTurn(async () => {
      this.registry.remove(id, file);
      let removed = false;
      for (const [key, input] of this.#inputs) {
        if (file ? cleanModuleId(input.id) === cleanModuleId(id) : input.id === id) {
          this.#inputs.delete(key);
          if (this.#designSystem.initialized) {
            this.#designSystem.compiler.native.remove(input.moduleId);
          }
          removed = true;
        }
      }
      return removed;
    });
  }

  /**
   * Reserves identifiers from authored CSS before generating conflicting compiler identifiers.
   *
   * @param id - Identity of the authored stylesheet. Its query is ignored for the compiler module
   * path.
   * @param code - Authored CSS text containing identifiers to reserve.
   * @param context - Resolver and watch operations used if the design system needs initialization.
   * @returns A promise that resolves after identifiers are registered in the native compiler.
   */
  reserveAuthoredCss(id: string, code: string, context: CssContext) {
    return this.#inTurn(async () => {
      await this.#designSystem.ensure(context, () => {
        this.revision++;
      });
      this.#designSystem.compiler.native.reserveAuthoredCss(
        compilerModuleId(this.settings.projectRoot, cleanModuleId(id)),
        code,
      );
    });
  }

  /**
   * Plans document styles for automatic producers after applying pending CSS changes.
   *
   * @param context - Resolver and watch operations for resources referenced by the selected styles.
   * @param complete - Whether the caller has collected the complete application graph. Defaults to
   * observed coverage.
   * @param selection - Optional compiler module paths without query suffixes. Omission selects all
   * automatic inputs. An empty list selects none.
   * @returns A promise for an ordered stylesheet with resolved resources and browser-ready parts.
   */
  documentPlan(context: CssContext, complete = false, selection?: readonly string[]) {
    return this.#inTurn(async () => {
      await this.#refresh(context);
      return this.#plan(this.#documentModules(selection), context, complete);
    });
  }

  /**
   * Plans styles for explicitly selected compiler modules.
   *
   * @param modules - Compiler module identities in selection order, including manual producers when
   * needed.
   * @param context - Resolver and watch operations for referenced resources.
   * @param complete - Whether the selection comes from a complete application graph. Defaults to
   * observed coverage.
   * @returns A promise for the stylesheet and its ordered parts after any design-system refresh.
   */
  modulePlan(modules: string[], context: CssContext, complete = false) {
    return this.#inTurn(async () => {
      await this.#refresh(context);
      return this.#plan(modules, context, complete);
    });
  }

  /**
   * Builds a development snapshot without incrementing the publication revision.
   *
   * @param context - Resolver and watch operations for the current stylesheet.
   * @returns A promise for document parts and producer dependencies at the current revision.
   */
  snapshot(context: CssContext) {
    return this.#inTurn(async () => {
      await this.#refresh(context);
      return this.#snapshot(await this.#plan(this.#documentModules(), context));
    });
  }

  /**
   * Advances the publication revision and delivers a current document stylesheet snapshot.
   *
   * The operation is serialized with compilation. A planning failure rejects before the callback is
   * called.
   *
   * @param context - Resolver and watch operations for stylesheet planning.
   * @param send - Receives the snapshot if its revision is still current when planning finishes.
   * @returns A promise that resolves after planning and any delivery.
   */
  publish(context: CssContext, send: (snapshot: CssSnapshot) => void) {
    return this.#inTurn(async () => {
      await this.#refresh(context);
      const revision = ++this.revision;
      const plan = await this.#plan(this.#documentModules(), context);
      if (revision === this.revision) {
        send(this.#snapshot(plan));
      }
    });
  }

  /**
   * Consumes producer identities changed by design-system reloads since the previous call.
   *
   * @returns A copy of the pending identities. The internal set is cleared.
   */
  takeThemeChanges(): Set<string> {
    const changes = new Set(this.#themeChanges);
    this.#themeChanges.clear();
    return changes;
  }

  /**
   * Refreshes the design system and records compiler modules requiring JavaScript invalidation.
   *
   * @param context - Resolver and watch operations for CSS reloads.
   * @returns A promise that resolves after refresh and invalidation callbacks.
   */
  async #refresh(context: CssContext) {
    await this.#designSystem.refresh(
      context,
      () => {
        this.revision++;
      },
      (modules) => {
        for (const module of modules) {
          this.#themeChanges.add(module);
        }
        if (modules.length) {
          this.invalidate(
            [...this.#inputs.values()]
              .filter((input) => modules.includes(input.moduleId))
              .map((input) => input.id),
          );
        }
      },
    );
  }

  /**
   * Selects automatic producer identities for document stylesheet planning.
   *
   * @param selection - Optional compiler module paths without query suffixes. An empty list selects
   * no inputs.
   * @returns Compiler module identities from nonmanual inputs, in their recorded order.
   */
  #documentModules(selection?: readonly string[]): string[] {
    return [...this.#inputs.values()]
      .filter(
        (input) =>
          !input.manual && (!selection || selection.includes(cleanModuleId(input.moduleId))),
      )
      .map((input) => input.moduleId);
  }

  /**
   * Resolves a stylesheet using the initialized compiler and current environment revision.
   *
   * @param modules - Compiler module identities selected for the stylesheet.
   * @param context - Resolver and watch operations for referenced resources.
   * @param complete - Whether to claim complete application coverage for this selection.
   * @returns A promise for resolved CSS and its ordered parts.
   */
  #plan(modules: string[], context: CssContext, complete = false) {
    return resolveStylesheet(
      this.#designSystem,
      this.settings,
      this.browserSources,
      this.revision,
      modules,
      context,
      complete,
    );
  }

  /**
   * Adds the current composition graph to a resolved development stylesheet.
   *
   * @param plan - Resolved stylesheet plan whose revision and parts are ready for the browser.
   * @returns A snapshot containing the plan's revision and parts plus dependencies from all
   * recorded inputs.
   */
  #snapshot(plan: StylesheetPlan): CssSnapshot {
    return {
      revision: plan.revision,
      parts: plan.parts,
      dependencies: Object.fromEntries(
        [...this.#inputs.values()].map((input) => [input.moduleId, input.dependencies]),
      ),
    };
  }

  /**
   * Runs an operation after earlier work and always releases the queue afterward.
   *
   * The queue release is independent of the operation result, so one failed operation does not
   * block later work.
   *
   * @param operation - Asynchronous compiler or registry operation. It must not enqueue and await
   * another environment operation.
   * @returns A promise for the operation's result, preserving any rejection.
   */
  async #inTurn<T>(operation: () => Promise<T>): Promise<T> {
    const previous = this.#work;
    let release!: () => void;
    this.#work = new Promise<void>((resolve) => {
      release = resolve;
    });
    await previous;

    try {
      return await operation();
    } finally {
      release();
    }
  }
}
