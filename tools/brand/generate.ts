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
write("apps/website/logo-shape-only.svg", markOnly(logo));
write("apps/website/logo-with-text.svg", lockupOutlined(logo));
write("tools/brand/output/stillwind-logo-outlined.svg", lockupOutlined(logo));
write("tools/brand/output/stillwind-logo-editable.svg", lockupEditable(logo, LOCKUP.tracking));

const website: Omit<OgImage, "width" | "height"> = {
  title: ["Keep your styles in order —", "even when AI writes them."],
  description: ["Tailwind CSS v4 utilities, checked and compiled."],
  field: "scattered",
};
// Each entry is [PNG path, SVG copy in tools/brand/output, content].
const images: [string, string, OgImage][] = [
  ["apps/website/public/og.png", "website-og.svg", { ...website, width: 1200, height: 630 }],
  [
    "apps/website/github-social-preview.png",
    "github-social-preview.svg",
    { ...website, width: 1280, height: 640 },
  ],
  [
    "apps/play/public/og.png",
    "play-og.svg",
    {
      title: ["Try Stillwind in your browser."],
      description: ["Write components and see the CSS as you type."],
      field: "brackets",
      width: 1200,
      height: 630,
    },
  ],
];
for (const [path, copy, image] of images) {
  const svg = ogSvg(image, logo);
  write(`tools/brand/output/${copy}`, svg);
  write(
    path,
    await sharp(Buffer.from(svg)).png({ compressionLevel: 9, palette: false }).toBuffer(),
  );
}
