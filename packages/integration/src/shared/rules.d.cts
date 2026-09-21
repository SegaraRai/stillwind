export interface StillwindOptions {
  css: string;
  automatic?: boolean;
  root?: string;
  prefix?: string;
  tailwindPrefix?: string;
  classNamespace?: string;
  forbidTailwindRelationships?: boolean;
}
export declare function stillwindRules(options: StillwindOptions): object[];
