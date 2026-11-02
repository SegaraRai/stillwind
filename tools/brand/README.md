# Brand

This directory contains the sources of the Stillwind logo, the favicons, and the social preview images.
`generate.ts` writes every file listed below, so edit the sources here instead of the generated files.

## Files

| File           | Contents                                                                                                       |
| -------------- | -------------------------------------------------------------------------------------------------------------- |
| `geometry.ts`  | Mark grid, tile shapes, colors, and the lockup layout. HarfBuzz positions text, and `opentype.js` outlines it. |
| `logo.ts`      | Logo icon layers, the favicon, and the logo files for distribution.                                            |
| `og.ts`        | Social preview images.                                                                                         |
| `generate.ts`  | Writes all generated files.                                                                                    |
| `preview.html` | Shows the generated files in light and dark color schemes.                                                     |

## Generated files

- `apps/{website,play}/src/icons/stillwind/{t,e,o}.svg`: icon layers of the `stillwind-logo` utility.
- `apps/{website,play}/public/favicon.svg` and `apps/play/src/icons/stillwind/mark.svg`: the mark.
  An embedded `prefers-color-scheme` media query selects the light or dark colors.
- `apps/website/public/og.png` and `apps/play/public/og.png`: the social preview images of the sites.
- `packages/vscode-extension/icon.png`: the mark with the light colors and a transparent background.

`tools/brand/output/` holds the files that are not part of an app or a package:

- `mark-light.svg` and `mark-dark.svg`: the mark in the light and dark colors.
- `mark-{light,dark}-250.png` and `mark-{light,dark}-500.png`: the mark with a transparent background. The VS Code
  Marketplace publisher uses `mark-light-250.png`, and the GitHub organization uses `mark-light-500.png`.
- `logo-outlined-light.svg` and `logo-outlined-dark.svg`: the lockup with outlined text in the light and dark
  colors. The root `README.md` selects the dark file with a `<picture>` element when GitHub uses a dark theme.
- `logo-editable.svg`: the lockup with the light colors and an editable Rubik `<text>` wordmark.
- `social-preview-{github,playground,website}.{svg,png}`: the social preview images. The GitHub image is uploaded
  in the repository settings, and the PNG files of the sites are the same as their `og.png`.

## Logo construction

All lengths use the mark grid. The mark's square is 32 units wide, and one unit is 0.5px in a 16px favicon.

- Each row is 8 units high, and rows and tiles are separated by 2 units. Tiles slant by 1/4 and have a corner
  radius of 0.75.
- The middle row is one flagged tile. The top and bottom rows each hold a short tile A and a long tile 3A.
  The top row moves 2 units right and the bottom row 2 units left, so the mark reads as an S.
- The wordmark is Rubik 400 with a cap height of 21 and letter spacing of -0.03em. The S starts 9 units right of
  the mark. The cap band is centered on the mark and then moved down by 1.

The `stillwind-logo` utility masks the element with `t`, which contains the wordmark and a rectangle over the mark.
The element's background is transparent for the left 25% of its width, and the `e` and `o` pseudo-elements draw
the tiles. If the lockup width changes, keep the `25%` in `apps/website/src/styles/site.css` and
`apps/play/src/styles.css` equal to `LAYER_SPLIT` in `logo.ts`. The logo colors in `site.css` and
`apps/play/src/theme.css` must also match `COLORS` in `geometry.ts`.

## Usage

```sh
vp run @stillwind-internal/tools-brand#generate:brand
```

Review the generated files before committing them. To view them, serve the repository root over HTTP and open
`/tools/brand/preview.html`.
