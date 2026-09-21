import type { StillwindOptions } from "../shared/rules.cjs";
export type { StillwindOptions } from "../shared/rules.cjs";
export declare function pluginStillwind(options: StillwindOptions): {
  name: string;
  setup(api: { modifyRspackConfig(callback: (config: any) => any): void }): void;
};
