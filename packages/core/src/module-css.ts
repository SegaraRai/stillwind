/** Replaced with a module-specific CSS value by the compiler integration. */
function missingCompiler(): string {
  throw new Error("stillwind/module-css requires a Stillwind compiler integration");
}

/** One immutable CSS payload at a position in the component's stylesheet. */
export interface CssPart {
  readonly key: string;
  readonly css: string;
}

export const css: string = missingCompiler();
export const parts: readonly CssPart[] = [];
export default css;
