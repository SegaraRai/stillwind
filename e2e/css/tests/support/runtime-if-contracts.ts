import fs from "node:fs/promises";
import path from "node:path";

import { stillwind } from "@stillwind/integration/vite";
import { build, preview } from "vite";
import { expect } from "vite-plus/test";

import type { RuntimeIfApplication } from "./runtime-if-application.ts";

export async function expectComputedConditions(
  app: RuntimeIfApplication,
  useCssIf: boolean,
): Promise<void> {
  const { dir, originalSource, page } = app.state;

  await fs.writeFile(
    path.join(dir, "main.ts"),
    originalSource
      .replace(
        "__NEGATED_VARIANTS__",
        useCssIf ? "grid if(!--enabled):flex if(!--enabled):if(--second):opacity-50" : "grid",
      )
      .replace(
        "__COMPARISON_UTILITIES__",
        useCssIf
          ? "grid if(--mode:compact):flex w-[--if(--mode:compact,40px,80px)] if(--mode:compact):if(--second):opacity-50"
          : "grid w-[80px]",
      ),
  );
  await build({
    configFile: false,
    root: dir,
    logLevel: "silent",
    plugins: [
      stillwind({
        css: path.join(dir, "root.css"),
        useCssIf,
      }),
    ],
    build: {
      minify: true,
      cssMinify: true,
      outDir: `dist-${useCssIf}`,
    },
  });

  const server = await preview({
    configFile: false,
    root: dir,
    logLevel: "silent",
    build: {
      outDir: `dist-${useCssIf}`,
    },
    preview: {
      port: 0,
    },
  });
  try {
    await page.goto(server.resolvedUrls!.local[0]);
    await page.waitForSelector("#sample");
    for (const [enabled, second] of [
      [false, false],
      [true, false],
      [true, true],
      [false, true],
    ]) {
      const actual = await page.evaluate(
        ({ enabled, second }) => {
          const element = document.querySelector<HTMLElement>("#sample")!;
          element.style.setProperty("--enabled", enabled ? " " : "initial");
          element.style.setProperty("--second", second ? " " : "initial");
          element.style.setProperty("--mode", enabled ? "compact" : "expanded");
          const style = getComputedStyle(element);
          return {
            display: style.display,
            width: style.width,
            height: style.height,
            opacity: style.opacity,
            stacked: getComputedStyle(document.querySelector<HTMLElement>("#stacked")!).display,
            before: getComputedStyle(element, "::before").content,
            inverseWidth: getComputedStyle(document.querySelector<HTMLElement>("#inverse")!).width,
            inverseDisplay: getComputedStyle(document.querySelector<HTMLElement>("#inverse")!)
              .display,
            inverseOpacity: getComputedStyle(document.querySelector<HTMLElement>("#inverse")!)
              .opacity,
            comparisonWidth: getComputedStyle(document.querySelector<HTMLElement>("#comparison")!)
              .width,
            comparisonDisplay: getComputedStyle(document.querySelector<HTMLElement>("#comparison")!)
              .display,
            comparisonOpacity: getComputedStyle(document.querySelector<HTMLElement>("#comparison")!)
              .opacity,
            authoredColor: getComputedStyle(document.querySelector<HTMLElement>("#authored")!)
              .color,
            authoredWidth: getComputedStyle(document.querySelector<HTMLElement>("#authored")!)
              .width,
            nestedColor: getComputedStyle(document.querySelector<HTMLElement>("#authored-nested")!)
              .color,
            nestedWidth: getComputedStyle(document.querySelector<HTMLElement>("#authored-nested")!)
              .width,
            nestedPseudo: getComputedStyle(
              document.querySelector<HTMLElement>("#nested-pseudo")!,
              "::before",
            ).content,
            nestedPseudoColor: getComputedStyle(
              document.querySelector<HTMLElement>("#nested-pseudo")!,
              "::before",
            ).color,
          };
        },
        { enabled, second },
      );
      expect(actual, JSON.stringify({ useCssIf, enabled, second })).toEqual({
        display: enabled ? "flex" : "grid",
        width: enabled ? "120px" : "60px",
        height: enabled ? (second ? "30px" : "20px") : "10px",
        opacity: enabled && second ? "0.5" : "1",
        stacked: second ? "flex" : enabled ? "block" : "grid",
        before: enabled ? '"yes"' : "none",
        inverseWidth: enabled ? "80px" : "40px",
        inverseDisplay: useCssIf && !enabled ? "flex" : "grid",
        inverseOpacity: useCssIf && !enabled && second ? "0.5" : "1",
        comparisonWidth: useCssIf && enabled ? "40px" : "80px",
        comparisonDisplay: useCssIf && enabled ? "flex" : "grid",
        comparisonOpacity: useCssIf && enabled && second ? "0.5" : "1",
        authoredColor: enabled ? (second ? "rgb(0, 0, 255)" : "rgb(255, 0, 0)") : "rgb(0, 0, 0)",
        authoredWidth: second ? "30px" : "10px",
        nestedColor: enabled ? (second ? "rgb(0, 0, 255)" : "rgb(255, 0, 0)") : "rgb(0, 0, 0)",
        nestedWidth: enabled ? "30px" : "10px",
        nestedPseudo: enabled ? '"yes"' : "none",
        nestedPseudoColor: enabled ? "rgb(255, 0, 0)" : "rgb(0, 0, 0)",
      });
    }
    console.log(`useCssIf=${useCssIf}: production minification and all four boolean states passed`);
  } finally {
    await new Promise<void>((resolve, reject) =>
      server.httpServer.close((error) => (error ? reject(error) : resolve())),
    );
  }
}
