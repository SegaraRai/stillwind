# @stillwind/host

[Stillwind](https://stillwind.dev/) compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when AI agents write much of it.

`@stillwind/host` provides the host services shared by Stillwind's build integrations and analysis tools:
CSS loading, semantic compilation, editor queries, output finalization, and source maps.

See [Development](https://github.com/SegaraRai/stillwind/blob/main/packages/host/DEVELOPMENT.md)
for module responsibilities and preview lifetimes, and
[Analysis contracts](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md)
for configuration and service embedding.
