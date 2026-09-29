# CSS minification

`stillwind-css` provides final asset minification through Lightning CSS with the
`minify` Cargo feature. The operation is independent of compilation and project
selection. Call it after expanding Stillwind directives, selecting the asset's
rules, and resolving resource URLs.

```rust
use stillwind_css::{CssMinifyOptions, minify_css};

let result = minify_css(css, &CssMinifyOptions {
    filename: "app.css".into(),
    source_map: true,
    ..CssMinifyOptions::default()
})?;
```

The Node binding exposes the same service:

```ts
import { minifyCss } from "@stillwind/native";

const result = minifyCss(css, {
  filename: "app.css",
  targets: { chrome: 120 << 16, safari: (17 << 16) | (2 << 8) },
  sourceMap: true,
  inputSourceMap: upstreamMapJson,
});
// result.css, result.sourceMap (JSON string when requested), result.warnings
```

The existing bundler integrations choose their own minification path. Exporting
this function does not automatically apply it to compiler output.

## Options

| Rust field                | JavaScript field        | Default and contract                                              |
| ------------------------- | ----------------------- | ----------------------------------------------------------------- |
| `filename`                | `filename`              | `input.css`; input identity for diagnostics and maps              |
| `targets`                 | `targets`               | Unspecified; browser names mapped to packed versions              |
| `include`                 | `include`               | Empty; features to always transform                               |
| `exclude`                 | `exclude`               | Empty; features to preserve even for older targets                |
| `source_map`              | `sourceMap`             | `false`; return a separate v3 map                                 |
| `input_source_map`        | `inputSourceMap`        | Absent; v3 JSON mapping this exact input CSS to its sources       |
| `include_sources_content` | `includeSourcesContent` | `true`; include source text in the output map                     |
| `unused_symbols`          | `unusedSymbols`         | Empty; names the caller has established are unused                |
| `error_recovery`          | `errorRecovery`         | `false`; allow invalid input to be discarded with parser warnings |

Browser keys are `android`, `chrome`, `edge`, `firefox`, `ie`, `ios_saf`, `opera`,
`safari`, and `samsung`. Versions are `major << 16 | minor << 8 | patch`, with one
byte per component. Configuration lookup and Browserslist resolution belong to the
host. Unknown browser keys and values larger than 24 bits are errors.

Features use Lightning CSS's case-sensitive names: `Nesting`, `NotSelectorList`,
`DirSelector`, `LangSelectorList`, `IsSelector`, `TextDecorationThicknessPercent`,
`MediaIntervalSyntax`, `MediaRangeSyntax`, `CustomMediaQueries`, `ClampFunction`,
`ColorFunction`, `OklabColors`, `LabColors`, `P3Colors`, `HexAlphaColors`,
`SpaceSeparatedColorNotation`, `FontFamilySystemUi`, `DoublePositionGradients`,
`VendorPrefixes`, `LogicalProperties`, and `LightDark`. The groups `Selectors`,
`MediaQueries`, and `Colors` are also accepted. Unknown names and overlapping
include/exclude flags are errors, including overlap through group flags.

## Source maps and errors

An input map requires output mapping to be enabled. Ordinary v3 maps are accepted;
indexed maps are rejected. Input map `sourceRoot` is retained on the composed map.
Embedded `sourceMappingURL` comments are ignored. The host publishes the returned
map and attaches its URL to the CSS if needed. Neither CSS imports nor map URLs
are fetched by this API.

Mapping granularity, coordinate behavior, and CSS optimization follow Lightning
CSS and Parcel Source Map. The wrapper does not repair upstream map coordinates.
Merged rules can have coarser source attribution than the compiler's original
range mappings. The returned map is debugging metadata, not an exact edit map.

Rust errors distinguish options, parsing, minification, printing, and source-map
processing. NAPI converts them to JavaScript errors. Parser warnings are returned
as strings with locations when available. Recovery does not suppress failures in
later processing stages.
