# Editor snapshot immutability

Status: Deferred.

`EditorSnapshot` retains a `ModuleAnalysis` and requires callers to keep its inputs unchanged. The graph contains mutable arrays and objects. A `readonly` field prevents replacing the graph reference but does not prevent changes to its contents.

`AnalysisProject` exposes the same graph through `AnalysisResult.graph`. `BrowserProject` appends editor reports to the graph's diagnostic batches after constructing `EditorSnapshot`. The snapshot's source index and cached reports can therefore refer to data whose contents are not protected by the API.

Review the construction and publication boundary before changing this behavior. Determine whether report augmentation must finish before creating an editor snapshot, or whether reports should have a separate owner. Expose completed graphs through read-only types and review nested values and sorting configuration retained by the snapshot. Assess whether caller-owned references require copying or another ownership boundary; TypeScript read-only types alone do not prevent changes through mutable aliases.

Validate that hover, fixes, sorting, and reports use a consistent source revision and that later project updates do not change retained editor results. The current impact of graph mutation has not been reproduced as an editor failure.

Relevant code:

- [EditorSnapshot](../packages/host/src/editor/snapshot.ts)
- [ModuleAnalysis](../packages/host/src/compilation/compiler.ts)
- [AnalysisProject](../packages/host/src/project/analysis.ts)
- [BrowserProject](../packages/host/src/browser/index.ts)
