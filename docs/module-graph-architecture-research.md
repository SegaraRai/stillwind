# Module CSS transport

Each transformed producer imports a self-contained CSS carrier. The carrier contains enough CSS and source-map information for another worker or a cached build to reconstruct the stylesheet. Correct output does not depend on every producer transform running in the same process.

An aggregate stylesheet loaded before a delayed producer completes can omit that producer. The integration therefore keeps document CSS separate from module contributions. Carrier closures include their resource and support dependencies, and composition dependencies determine stylesheet ordering.

The host uses the project-relative module path for default class identity. A CSS dependency edit invalidates producers even when their generated JS and classes stay the same. Macro removal removes the corresponding carrier import. Explicit module CSS values let consumers control stylesheet placement, including Shadow DOM.

The implementation and public options are described in [module-graph-implementation.md](module-graph-implementation.md) and [the integration package](../packages/integration/README.md).

Current build-tool contracts are checked by `vp run @stillwind/integration#test`. Browser acceptance uses the real compiler and lives in [e2e/frameworks](../e2e/frameworks/README.md). Run `vp run verify:e2e` for its full matrix. Vite dependency edits may reload the page; the browser checks do not guarantee state-preserving HMR.
