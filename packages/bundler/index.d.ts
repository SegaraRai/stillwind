export interface StillwindBundlerOptions {
  css: string;
  root?: string;
  prefix?: string;
  tailwindPrefix?: string;
  classNamespace?: string;
  forbidTailwindRelationships?: boolean;
}
export declare function stillwindRules(options: StillwindBundlerOptions): object[];
export declare function withStillwind<T extends object>(
  config: T,
  options: StillwindBundlerOptions,
): T;
export declare function pluginStillwind(options: StillwindBundlerOptions): {
  name: string;
  setup(api: { modifyRspackConfig(callback: (config: any) => any): void }): void;
};
