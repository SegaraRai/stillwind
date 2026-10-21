# Shape Lab development

The editor uses the workspace Vite+ toolchain and does not require a compiler
build. Run from the repository root:

```sh
vp run @stillwind-internal/apps-logo-editor#dev
vp run @stillwind-internal/apps-logo-editor#test
vp run @stillwind-internal/apps-logo-editor#typecheck
vp run @stillwind-internal/apps-logo-editor#build
```

Development serves [http://127.0.0.1:5198/](http://127.0.0.1:5198/).
The [User guide](docs/usage.md) defines controls, persistence, geometry, and export behavior.

`src/geometry.ts` owns graph normalization, face discovery, inset geometry, snapping,
presets, document validation, and SVG serialization. `src/main.ts` owns canvas
interaction, history, local persistence, and the inspector. Geometry tests cover
topology changes, shared intersections, face paint inheritance, perpendicular
spacing, tapered widths, snap completion, and JSON round trips.
