/**
 * Rejects evaluation of an explicit CSS import that the compiler integration has not replaced.
 *
 * @returns No value. This function always throws.
 * @throws Always, because this runtime module cannot provide compiled CSS.
 */
function missingCompiler(): string {
  throw new Error("stillwind/module-css requires a Stillwind compiler integration");
}

/**
 * One compiled stylesheet occurrence, used for ordered CSS text and constructed stylesheets.
 */
export interface CssPart {
  /**
   * Cache identity for this occurrence. A document reuses its sheet while both key and CSS match.
   */
  readonly key: string;
  /**
   * CSS body of this occurrence, without notices. `cssText` from `stillwind/css` combines bodies
   * and adds their notices.
   */
  readonly css: string;
  /**
   * Notices, such as licenses, that this occurrence requires. A missing list means none.
   */
  readonly banners?: readonly string[];
}

/**
 * CSS text supplied when a compiler integration replaces this module. Evaluating this source module
 * throws before initialization completes.
 */
export const css: string = missingCompiler();

/**
 * Ordered CSS occurrences supplied by a compiler integration. The source placeholder is empty and
 * cannot be used without that replacement.
 */
export const parts: readonly CssPart[] = [];

/**
 * Default export of the compiled CSS text. The compiler integration must replace this module before
 * evaluation.
 */
export default css;
