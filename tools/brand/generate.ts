/**
 * Writes the logo, favicon, and social preview files of the apps from the sources in this directory.
 *
 * Usage: `node generate.ts`. Review the changed files before committing them.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

import sharp from "sharp";

import { LOCKUP, lockup } from "./geometry.ts";
import { lockupEditable, lockupOutlined, logoLayers, markIcon, markOnly } from "./logo.ts";
import { type OgImage, ogSvg } from "./og.ts";

const root = join(import.meta.dirname, "../..");
const write = (path: string, content: string | Buffer) => {
  const target = join(root, path);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, content);
  console.log(`wrote ${path}`);
};

const logo = lockup();
const layers = logoLayers(logo);
const mark = markIcon(logo);

for (const app of ["website", "play"]) {
  for (const [name, svg] of Object.entries(layers)) {
    write(`apps/${app}/src/icons/stillwind/${name}.svg`, svg);
  }
  write(`apps/${app}/public/favicon.svg`, mark);
}
write("apps/play/src/icons/stillwind/mark.svg", mark);
// stillwind.dev serves the logo and mark SVG files under /brand/ for other sites to use.
for (const scheme of ["light", "dark"] as const) {
  const svg = lockupOutlined(logo, scheme);
  write(`tools/brand/output/logo-outlined-${scheme}.svg`, svg);
  write(`apps/website/public/brand/logo-outlined-${scheme}.svg`, svg);
}
write("tools/brand/output/logo-editable.svg", lockupEditable(logo, LOCKUP.tracking));
// Square icons are drawn at 8x and scaled down so that their small tile gaps stay sharp.
const squarePng = async (svg: string, size: number) =>
  await sharp(Buffer.from(svg), {
    density: 72 * 8,
  })
    .resize(size, size, {
      kernel: "lanczos3",
    })
    .png({
      compressionLevel: 9,
      palette: false,
    })
    .toBuffer();
write("packages/vscode-extension/icon.png", await squarePng(markOnly(logo, "light"), 128));
for (const scheme of ["light", "dark"] as const) {
  const svg = markOnly(logo, scheme);
  write(`tools/brand/output/mark-${scheme}.svg`, svg);
  write(`apps/website/public/brand/mark-${scheme}.svg`, svg);
  // The VS Code Marketplace publisher uses the 250px icon, and the GitHub organization uses the 500px icon.
  for (const size of [250, 500]) {
    write(`tools/brand/output/mark-${scheme}-${size}.png`, await squarePng(svg, size));
  }
}

const website: Omit<OgImage, "width" | "height"> = {
  title: ["Keep your styles in order —", "even when AI writes them."],
  description: ["Tailwind CSS v4 utilities, checked and compiled."],
  field: "scattered",
};
// Each entry is [name in tools/brand/output, PNG path in an app or null, content].
const images: [string, string | null, OgImage][] = [
  [
    "website",
    "apps/website/public/og.png",
    {
      ...website,
      width: 1200,
      height: 630,
    },
  ],
  [
    "github",
    null,
    {
      ...website,
      width: 1280,
      height: 640,
    },
  ],
  [
    "playground",
    "apps/play/public/og.png",
    {
      title: ["Try Stillwind in your browser."],
      description: ["Write components and see the CSS as you type."],
      field: "brackets",
      width: 1200,
      height: 630,
    },
  ],
];
for (const [name, appPath, image] of images) {
  const svg = ogSvg(image, logo);
  const png = await sharp(Buffer.from(svg))
    .png({
      compressionLevel: 9,
      palette: false,
    })
    .toBuffer();
  write(`tools/brand/output/social-preview-${name}.svg`, svg);
  write(`tools/brand/output/social-preview-${name}.png`, png);
  if (appPath !== null) {
    write(appPath, png);
  }
}
