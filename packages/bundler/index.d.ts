export interface StillwindBundlerOptions {
  css: string;
  automatic?: boolean;
  root?: string;
  prefix?: string;
  tailwindPrefix?: string;
  classNamespace?: string;
  forbidTailwindRelationships?: boolean;
}
export declare function stillwindRules(options: StillwindBundlerOptions): object[];
/** Installs complete-graph emission for configurations using stillwindRules. */
export declare class StillwindPlugin {
  apply(compiler: any): void;
}
export declare function withStillwind<T extends object>(
  config: T,
  options: StillwindBundlerOptions,
): T;
export declare function pluginStillwind(options: StillwindBundlerOptions): {
  name: string;
  setup(api: { modifyRspackConfig(callback: (config: any) => any): void }): void;
};
