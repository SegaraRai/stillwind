declare const scopeIdentity: unique symbol;
declare const refIdentity: unique symbol;

export interface Scope<Id extends string> {
  readonly [scopeIdentity]: Id;
}

export interface Ref<Id extends string, ScopeId extends string = never> {
  readonly [refIdentity]: {
    readonly ref: Id;
    readonly scope: ScopeId;
  };
}

export interface DeclarationIdentity<Id extends string> {
  readonly __stillwindDeclarationIdentity: Id;
}

export interface StyleFactory {
  scope<const Id extends string = string>(): Scope<Id>;
  ref(scope?: Scope<string>): Ref<string, string>;
  ref<const ScopeId extends string, const Id extends string>(
    scope: Scope<ScopeId> | undefined,
    identity: DeclarationIdentity<Id>,
  ): Ref<Id, ScopeId>;
}

export declare const style: StyleFactory;
