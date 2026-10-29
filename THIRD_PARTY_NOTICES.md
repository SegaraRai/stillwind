# Third-party notices

This file lists third-party material in this repository and in the published
packages. Stillwind itself is licensed under the [MIT License](LICENSE).

## Tailwind CSS v4.3.3

Tailwind CSS is licensed under the MIT License. Its license text is in
[TAILWINDCSS-MIT-LICENSE](TAILWINDCSS-MIT-LICENSE). The packages that contain
material from Tailwind CSS include that file: `stillwind`, the platform packages
of `@stillwind/native`, and the VS Code extension.

The `stillwind` package contains these files:

- `packages/core/css/preflight.css` and `packages/core/css/theme.css` are adapted
  from `preflight.css` and `theme.css` of Tailwind CSS v4.3.3.
- The files whose names start with `tw-` in `packages/core/css/builtin-utilities/`,
  `packages/core/css/builtin-variants/`, and `packages/core/css/builtin-metadata/`
  define the utilities and variants of Tailwind CSS as Stillwind CSS definitions.
  They follow `src/utilities.ts` and `src/variants.ts` of Tailwind CSS v4.3.3.

The native binaries contain this code and data:

- `crates/design-system/src/evaluation/utilities/tailwind_property_order.rs`
  contains the property names of `src/property-order.ts`, in source order.
  `tools/tailwind/utility-order.ts` generates it. The adjacent `order.rs`
  implements the comparisons of `src/compile.ts` and `src/utils/compare.ts`.
- `crates/compiler/src/banner/tailwind.rs` contains the license notice of Tailwind
  CSS. `tools/tailwind/banner.ts` generates it from the Tailwind compiler. Stillwind
  adds the notice to the stylesheets that it publishes.

The repository also contains test data that the packages do not include:

- `fixtures/tailwind-v4/` contains compatibility cases, whose expected CSS the
  Tailwind compiler generates, and catalogs of utilities and variants, which
  `tools/tailwind/registrations.ts` generates from the Tailwind sources.
  `fixtures/tailwind-v4/utility-order.json` records the utility order that the
  Tailwind compiler produces.
- The cases in `fixtures/canonical/` are adapted from
  `src/canonicalize-candidates.test.ts`.
- `crates/test-support/assets/` contains bundles of the built-in CSS, including
  the adapted preflight and theme.

`tools/tailwind/upstream-source.json` records the Tailwind CSS version and revision
that the generated files and fixtures use.

## @webref/css 8.7.8

`crates/css/assets/webref-properties.rs` contains the property names, value
syntaxes, and specification links of `css.json` in
[@webref/css](https://github.com/w3c/webref) 8.7.8. `tools/webref/webref.ts`
generates it. The native binaries contain this data.

Webref is licensed under the MIT License. Its license text is in
[WEBREF-MIT-LICENSE](WEBREF-MIT-LICENSE). The platform packages of
`@stillwind/native` and the VS Code extension include that file.

## Rust crates

The native binaries link third-party Rust crates.
`packages/native/RUST-THIRD-PARTY-LICENSES` contains their license texts, and
`vp run @stillwind/native#generate:rust-licenses` generates it from Cargo metadata.
The platform packages of `@stillwind/native` and the VS Code extension include that
file. `vp run @stillwind/native#prepare-publish` copies it into the platform packages.

## Bundled npm packages

Some packages bundle third-party npm packages into their JavaScript. Their builds
write the license texts of those packages to `NPM-THIRD-PARTY-LICENSES`:

- `@stillwind/integration` publishes it in `dist/`.
- The VS Code extension publishes it at the root of the extension. The desktop
  extension also lists the dependencies that it copies into `dist/node/node_modules/`.

## Fonts of the website

The website serves the Instrument Sans font from `@fontsource/instrument-sans` and
the JetBrains Mono font from `apps/website/src/assets/fonts/`. Both fonts are
licensed under the SIL Open Font License 1.1, and the website publishes their
license texts from `apps/website/public/fonts/`. The packages do not include these
fonts.
