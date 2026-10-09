declare module "virtual:stillwind/astro-styles" {
  /**
   * Reads markup for the active SSR environment's document stylesheet.
   * @returns Inline styles and an HMR script in development, or manifest stylesheet links in production.
   * @throws If the development project is unavailable or stylesheet planning fails.
   */
  export function getStyles(): Promise<string>;
}
