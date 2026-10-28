import { useEffect, useState, type Ref } from "react";
import { style } from "stillwind/react";

import type { ApplicationBuild } from "../build/client.ts";

/**
 * Content of the result pane.
 */
export type ResultTab = "preview" | "css" | "code" | "bundle";

/**
 * Width of the preview frame. `desktop` fills the pane.
 */
export type Viewport = "desktop" | "tablet" | "mobile";

/**
 * Text that the output view shows for a non-preview tab.
 */
export interface Output {
  /** Stable model identifier. Each tab and file pair keeps its own model. */
  id: string;
  /** VS Code language ID used for highlighting. */
  language: string;
  /** Complete text, or a comment that explains why no output exists. */
  text: string;
  /** Whether `text` is build output that the copy button and size label describe. */
  generated: boolean;
}

const TABS: [ResultTab, string][] = [
  ["preview", "Preview"],
  ["css", "Generated CSS"],
  ["code", "Compiled Code"],
  ["bundle", "Bundled CSS"],
];

const VIEWPORTS: [Viewport, string][] = [
  ["desktop", "Desktop"],
  ["tablet", "Tablet"],
  ["mobile", "Mobile"],
];

/**
 * Selects the output for a non-preview tab.
 *
 * @param tab - `css` and `code` describe the file open in the editor. `bundle` describes the
 *   complete stylesheet of the preview build.
 * @param file - Absolute virtual path of the file open in the editor.
 * @param result - Latest successful build, or `undefined` before the first build.
 */
export function selectOutput(
  tab: Exclude<ResultTab, "preview">,
  file: string,
  result: ApplicationBuild | undefined,
): Output {
  const css = (text: string, generated = false): Output => ({
    id: `${tab}:${file}`,
    language: "css",
    text,
    generated,
  });
  if (!result) {
    return css("/* The first build has not finished yet. */\n");
  }
  if (tab === "bundle") {
    return {
      ...css(result.css, true),
      id: "bundle",
    };
  }
  const name = file.replace("/workspace/", "");
  if (!/\.[cm]?[jt]sx?$/.test(file)) {
    return css(`/* ${name} is not a JavaScript or TypeScript module. */\n`);
  }
  if (tab === "css") {
    const value = new Map(result.moduleCss).get(file);
    return value === undefined
      ? css(
          `/* ${name} has no generated CSS because it is not reachable from the preview entry. */\n`,
        )
      : value
        ? css(value, true)
        : css(`/* ${name} generates no CSS. */\n`);
  }
  const code = new Map(result.transformed).get(file);
  return {
    id: `${tab}:${file}`,
    language: scriptLanguage(file),
    text:
      code ??
      `// ${name} has no compiled output because it is not reachable from the preview entry.\n`,
    generated: code !== undefined,
  };
}

/**
 * Returns the VS Code language ID of a JavaScript or TypeScript module.
 */
function scriptLanguage(file: string): string {
  const typescript = /\.[cm]?tsx?$/.test(file);
  const jsx = file.endsWith("x");
  return typescript
    ? jsx
      ? "typescriptreact"
      : "typescript"
    : jsx
      ? "javascriptreact"
      : "javascript";
}

/**
 * Shows the preview frame or a build output, and the latest build or runtime error.
 * The frame and the output host stay mounted while hidden, so the preview keeps its React state.
 *
 * @param props.frame - Receives the preview iframe. The caller owns its document and channel.
 * @param props.outputHost - Receives the element that the caller's read-only editor fills.
 * @param props.output - Output of the selected tab, or `undefined` while the preview is shown.
 * @param props.error - Message of the latest failed build or preview execution.
 */
export function ResultPane({
  tab,
  onTab,
  viewport,
  onViewport,
  frame,
  outputHost,
  output,
  error,
}: {
  tab: ResultTab;
  onTab: (tab: ResultTab) => void;
  viewport: Viewport;
  onViewport: (viewport: Viewport) => void;
  frame: Ref<HTMLIFrameElement>;
  outputHost: Ref<HTMLDivElement>;
  output: Output | undefined;
  error: string | undefined;
}) {
  return (
    <section
      aria-label="Result"
      {...style("relative flex min-block-0 min-inline-0 flex-col bg-editor")}
    >
      <div {...style("flex block-9 shrink-0 items-stretch bg-background border-divider border-be")}>
        <div
          role="tablist"
          aria-label="Result views"
          {...style(
            "flex overflow-x-auto min-inline-0 flex-1 items-stretch scrollbar-none select-none",
          )}
        >
          {TABS.map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="tab"
              aria-selected={tab === id}
              onClick={() => onTab(id)}
              {...style("workbench-tab")}
            >
              <span
                aria-hidden="true"
                {...style(
                  "icon-inline",
                  {
                    preview: "icon-lucide--app-window",
                    css: "icon-lucide--hash",
                    code: "icon-lucide--file-code",
                    bundle: "icon-lucide--layers",
                  }[id],
                )}
              />
              {label}
            </button>
          ))}
        </div>
        <div {...style("flex px-2 shrink-0 items-center justify-end gap-1")}>
          {tab === "preview" ? (
            <div {...style("flex items-center gap-1 max-inline:hidden")}>
              {VIEWPORTS.map(([id, label]) => (
                <button
                  key={id}
                  type="button"
                  title={label}
                  aria-label={label}
                  aria-pressed={viewport === id}
                  onClick={() => onViewport(id)}
                  {...style("icon-button")}
                >
                  <span
                    aria-hidden="true"
                    {...style(
                      "icon-control",
                      {
                        desktop: "icon-lucide--monitor",
                        tablet: "icon-lucide--tablet",
                        mobile: "icon-lucide--smartphone",
                      }[id],
                    )}
                  />
                </button>
              ))}
            </div>
          ) : (
            output?.generated && (
              <>
                <span {...style("me-1 text-caption text-subtle whitespace-nowrap")}>
                  {formatSize(output.text)}
                </span>
                <CopyButton text={output.text} />
              </>
            )
          )}
        </div>
      </div>
      {error && (
        <div
          role="alert"
          {...style(
            "flex overflow-auto px-3 py-2 max-block-40 shrink-0 gap-2 text-caption text-foreground bg-danger/10 border-danger/40 border-be",
            { class: "runtime-error" },
          )}
        >
          <span
            aria-hidden="true"
            {...style("icon-control icon-lucide--circle-x mbs-px shrink-0 text-danger")}
          />
          <pre {...style("min-inline-0 wrap-anywhere whitespace-pre-wrap code-font")}>{error}</pre>
        </div>
      )}
      <div
        {...style(
          "overflow-auto p-4 min-block-0 flex-1 place-items-center stage-grid",
          tab === "preview" ? "grid" : "hidden",
        )}
      >
        <iframe
          ref={frame}
          title="Stillwind Play preview"
          sandbox="allow-forms allow-scripts"
          {...style(
            "block-full inline-full min-block-70 bg-frame rounded-panel shadow-overlay [transition:max-inline-size_200ms]",
            {
              desktop: "max-inline-full",
              tablet: "max-inline-tablet",
              mobile: "max-inline-phone",
            }[viewport],
          )}
        />
      </div>
      <div
        ref={outputHost}
        {...style("min-block-0 flex-1", tab === "preview" ? "hidden" : "block")}
      />
    </section>
  );
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) {
      return;
    }
    const timer = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(timer);
  }, [copied]);
  return (
    <button
      type="button"
      title={copied ? "Copied" : "Copy"}
      aria-label={copied ? "Copied" : "Copy"}
      onClick={async () => {
        await navigator.clipboard.writeText(text);
        setCopied(true);
      }}
      {...style("icon-button")}
    >
      <span
        aria-hidden="true"
        {...style("icon-control", copied ? "icon-lucide--check text-success" : "icon-lucide--copy")}
      />
    </button>
  );
}

function formatSize(text: string): string {
  const bytes = new TextEncoder().encode(text).length;
  return bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} kB`;
}
