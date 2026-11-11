/**
 * Options that let Stillwind resolve a server-only source from a client module. React Router's
 * `react-router:dot-server` plugin skips its client import check when it receives the `custom`
 * flag. Stillwind reads such a source only for type and style facts, and Vite still checks the
 * imports that remain in the transformed client code.
 */
interface ServerSourceResolveOptions {
  /** Avoids re-entering the Stillwind plugin that initiated resolution. */
  skipSelf: true;
  /** Flag read by React Router's `react-router:dot-server` resolver hook. */
  custom: { "react-router:dot-server": true };
}

/** Text of the error that React Router throws when a client module resolves a server-only source. */
const serverOnlyImportMessage = "Server-only module referenced by client";

/** Number of resolutions before a React Router server-only import error is passed to the caller. */
const serverSourceResolveAttempts = 5;

/**
 * Resolves an import whose target may be a server-only source, through the other Vite plugins.
 *
 * Rolldown can deliver the `custom` option of one `this.resolve()` call to a different call that
 * runs at the same time (https://github.com/rolldown/rolldown/issues/11180). It stores the options
 * of each pending call under the current size of a map. After an earlier call finishes, a new call
 * can receive the key of a pending call and replace its options. React Router then does not see
 * the flag from {@link ServerSourceResolveOptions} and throws for a valid Stillwind read, for example
 * `../server.server.js?stillwind.css` in a route module. This retry is a workaround and is not
 * needed after Rolldown gives each pending call its own key. Other plugin errors and other
 * failures are passed to the caller without a retry.
 *
 * @param resolve - Resolver of the current Vite hook context. Each call must be independent, so
 * the same arguments can be resolved again. Each call receives a new options object.
 * @param specifier - Import text without the Stillwind resource query.
 * @param importer - Resolver identity of the importing module.
 * @returns The result of the first resolution that does not throw React Router's server-only
 * import error.
 * @throws The error of the last attempt if each attempt throws React Router's server-only import
 * error, or the first error of another kind.
 */
export async function resolveServerSource<T>(
  resolve: (specifier: string, importer: string, options: ServerSourceResolveOptions) => Promise<T>,
  specifier: string,
  importer: string,
): Promise<T> {
  for (let attempt = 1; ; attempt++) {
    try {
      return await resolve(specifier, importer, {
        skipSelf: true,
        custom: { "react-router:dot-server": true },
      });
    } catch (error) {
      if (attempt >= serverSourceResolveAttempts || !isServerOnlyImportError(error)) {
        throw error;
      }
    }
  }
}

/**
 * Checks whether a resolver error, or one of its causes, is React Router's server-only import error.
 *
 * @param error - Value thrown by a resolver. Rolldown can wrap the plugin error in another error.
 * @returns True when the message of the error or of a cause contains React Router's message.
 */
function isServerOnlyImportError(error: unknown): boolean {
  for (let current = error; current instanceof Error; current = current.cause) {
    if (current.message.includes(serverOnlyImportMessage)) {
      return true;
    }
  }
  return false;
}
