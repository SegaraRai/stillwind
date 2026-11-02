import type { Ref } from "react";
import { style } from "stillwind/react";
import type * as vscode from "vscode";

import type { ChannelEntry } from "../workspace/channels.ts";
import { ProblemList, type Problem } from "./problems.tsx";

/**
 * Content of the panel below the editor.
 */
export type PanelTab = "problems" | "output";

const counter = style(
  "px-1.5 min-inline-4.5 text-center text-foreground text-label font-semibold bg-surface-muted rounded-full",
);

/**
 * Shows the Problems list or the Output channels below the editor, like the panel of VS Code.
 * The output host stays mounted while hidden, so the caller's editor keeps its channel. Below the
 * inline breakpoint, the tabs are hidden and the caller selects `tab`.
 *
 * @param props.problems - Diagnostics in the order that the language service reports them.
 * @param props.onSelect - Called with the file and the zero-based range of a clicked diagnostic.
 * @param props.channels - Output channels in registration order.
 * @param props.channel - Identifier of the channel that the output host shows.
 * @param props.outputHost - Receives the element that the caller's read-only editor fills.
 */
export function Panel({
  tab,
  onTab,
  problems,
  onSelect,
  channels,
  channel,
  onChannel,
  outputHost,
}: {
  tab: PanelTab;
  onTab: (tab: PanelTab) => void;
  problems: Problem[];
  onSelect: (file: string, range: vscode.Range) => void;
  channels: ChannelEntry[];
  channel: string | undefined;
  onChannel: (id: string) => void;
  outputHost: Ref<HTMLDivElement>;
}) {
  return (
    <section
      aria-label="Panel"
      {...style("flex min-block-0 min-inline-0 flex-col bg-background border-divider border-bs")}
    >
      <div
        {...style("flex block-9 shrink-0 items-stretch", tab === "problems" && "max-inline:hidden")}
      >
        {/* Below the inline breakpoint, the view bar of the page selects the tab. */}
        <div
          role="tablist"
          aria-label="Panel views"
          {...style("flex min-inline-0 flex-1 items-stretch select-none max-inline:hidden")}
        >
          <button
            type="button"
            role="tab"
            aria-selected={tab === "problems"}
            onClick={() => onTab("problems")}
            {...style("workbench-tab")}
          >
            Problems
            <span {...counter}>{problems.length}</span>
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={tab === "output"}
            onClick={() => onTab("output")}
            {...style("workbench-tab")}
          >
            Output
          </button>
        </div>
        {tab === "output" && (
          <div {...style("flex px-2 shrink-0 items-center max-inline:flex-1")}>
            <select
              aria-label="Output channel"
              value={channel ?? ""}
              disabled={!channels.length}
              onChange={(event) => onChannel(event.target.value)}
              {...style(
                "px-2 block-6.5 max-inline-56 text-caption text-foreground bg-surface rounded-control border border-divider cursor-pointer disabled:cursor-default",
              )}
            >
              {channels.map(({ id, label }) => (
                <option key={id} value={id}>
                  {label}
                </option>
              ))}
            </select>
          </div>
        )}
      </div>
      {tab === "problems" && <ProblemList problems={problems} onSelect={onSelect} />}
      <div
        ref={outputHost}
        {...style("min-block-0 flex-1 bg-editor", tab === "output" ? "block" : "hidden", {
          class: "channel-output",
        })}
      />
    </section>
  );
}
