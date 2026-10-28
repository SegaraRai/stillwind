import { useState, type ReactNode } from "react";
import { style } from "stillwind/react";

import { fileIcon, getFileInfo } from "../file-info.ts";

const WORKSPACE = "/workspace/";

type TreeNode = {
  name: string;
  /** Absolute virtual path. Folder paths end without a slash. */
  path: string;
  /** Present only for folders. Folders come before files, and each group is sorted by name. */
  children?: TreeNode[];
};

function buildTree(files: string[]): TreeNode[] {
  const root: TreeNode = { name: "", path: WORKSPACE.slice(0, -1), children: [] };
  for (const file of files) {
    let parent = root;
    const segments = file.slice(WORKSPACE.length).split("/");
    segments.forEach((name, index) => {
      const path = `${parent.path}/${name}`;
      const leaf = index === segments.length - 1;
      let node = parent.children!.find((child) => child.name === name && !child.children === leaf);
      if (!node) {
        node = leaf ? { name, path } : { name, path, children: [] };
        parent.children!.push(node);
      }
      parent = node;
    });
  }
  const sort = (nodes: TreeNode[]) => {
    nodes.sort((a, b) => Number(!a.children) - Number(!b.children) || a.name.localeCompare(b.name));
    for (const node of nodes) {
      if (node.children) {
        sort(node.children);
      }
    }
    return nodes;
  };
  return sort(root.children!);
}

const row = style(
  "flex ps-(--indent) pe-2 block-5.5 inline-full items-center gap-1.5 text-start text-muted text-ui whitespace-nowrap cursor-pointer",
  {
    "hover:": "text-foreground bg-surface-muted/60",
    "aria-[current=page]:": "text-foreground bg-selection",
  },
);

/**
 * Lists workspace files as a tree and creates files with an inline name field.
 *
 * @param props.files - Absolute virtual paths under `/workspace/`.
 * @param props.active - Path of the file that the editor shows.
 * @param props.onSelect - Called with the absolute path of a clicked file.
 * @param props.onAdd - Creates a file from a path relative to the workspace. A rejection keeps
 *   the field open and shows the error message.
 */
export function Explorer({
  files,
  active,
  onSelect,
  onAdd,
}: {
  files: string[];
  active: string;
  onSelect: (file: string) => void;
  onAdd: (name: string) => Promise<void>;
}) {
  const [collapsed, setCollapsed] = useState(new Set<string>());
  const [draft, setDraft] = useState<string>();
  const [error, setError] = useState<string>();

  const submit = async () => {
    const name = draft?.trim();
    if (!name) {
      setDraft(undefined);
      return;
    }
    try {
      await onAdd(name);
      setDraft(undefined);
      setError(undefined);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const render = (nodes: TreeNode[], depth: number): ReactNode =>
    nodes.map((node) => {
      if (!node.children) {
        return (
          <li key={node.path}>
            <button
              type="button"
              aria-current={node.path === active ? "page" : undefined}
              onClick={() => onSelect(node.path)}
              {...style(row, { "--indent": `${0.5 + depth * 0.75}rem` })}
            >
              <span {...style("inline-4 shrink-0")} />
              <span
                data-type={getFileInfo(node.name).type}
                {...style(fileIcon, "icon-control shrink-0")}
              />
              <span {...style("truncate")}>{node.name}</span>
            </button>
          </li>
        );
      }
      const open = !collapsed.has(node.path);
      return (
        <li key={node.path}>
          <button
            type="button"
            aria-expanded={open}
            onClick={() => {
              const next = new Set(collapsed);
              if (open) {
                next.add(node.path);
              } else {
                next.delete(node.path);
              }
              setCollapsed(next);
            }}
            {...style(row, { "--indent": `${0.5 + depth * 0.75}rem` })}
          >
            <span
              aria-hidden="true"
              {...style(
                "icon-control icon-lucide--chevron-right shrink-0 [transition:rotate_100ms]",
                open && "rotate-90",
              )}
            />
            <span
              aria-hidden="true"
              {...style(
                "icon-control shrink-0 text-subtle",
                open ? "icon-lucide--folder-open" : "icon-lucide--folder",
              )}
            />
            <span {...style("truncate")}>{node.name}</span>
          </button>
          {open && <ul>{render(node.children, depth + 1)}</ul>}
        </li>
      );
    });

  return (
    <aside
      aria-label="Explorer"
      {...style("flex min-block-0 min-inline-0 flex-col bg-background select-none")}
    >
      <div {...style("flex ps-4 pe-2 block-9 items-center gap-1")}>
        <h2 {...style("pane-title")}>Explorer</h2>
        <span {...style("ms-auto")} />
        <button
          type="button"
          title="New File"
          aria-label="New File"
          onClick={() => {
            setDraft("");
            setError(undefined);
          }}
          {...style("icon-button")}
        >
          <span aria-hidden="true" {...style("icon-control icon-lucide--file-plus")} />
        </button>
        <button
          type="button"
          title="Collapse Folders"
          aria-label="Collapse Folders"
          onClick={() =>
            setCollapsed(new Set(files.flatMap((file) => folders(file.slice(WORKSPACE.length)))))
          }
          {...style("icon-button")}
        >
          <span aria-hidden="true" {...style("icon-control icon-lucide--chevrons-down-up")} />
        </button>
      </div>
      <div {...style("overflow-y-auto pbe-2 min-block-0 flex-1")}>
        <ul aria-label="Workspace files">
          {draft !== undefined && (
            <li {...style("relative")}>
              <form
                onSubmit={(event) => {
                  event.preventDefault();
                  void submit();
                }}
                {...style("flex px-2 block-5.5 items-center gap-1.5 select-auto")}
              >
                <span {...style("inline-4 shrink-0")} />
                <span
                  data-type={getFileInfo(draft).type}
                  {...style(fileIcon, "icon-control shrink-0")}
                />
                <input
                  autoFocus
                  aria-label="New file path"
                  aria-invalid={error ? true : undefined}
                  placeholder="components/Counter.tsx"
                  spellCheck={false}
                  value={draft}
                  onChange={(event) => {
                    setDraft(event.target.value);
                    setError(undefined);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Escape") {
                      setDraft(undefined);
                      setError(undefined);
                    }
                  }}
                  onBlur={() => {
                    if (!draft) {
                      setDraft(undefined);
                    }
                  }}
                  {...style(
                    "px-1 block-5.5 min-inline-0 flex-1 text-foreground text-ui bg-editor border border-accent outline-none placeholder:text-subtle",
                    { "aria-invalid:": "border-danger" },
                  )}
                />
              </form>
              {error && (
                <p
                  role="alert"
                  {...style(
                    "absolute inset-x-2 inset-bs-full z-10 px-2 py-1 text-caption text-foreground bg-surface border border-danger shadow-overlay",
                  )}
                >
                  {error}
                </p>
              )}
            </li>
          )}
          {render(buildTree(files), 0)}
        </ul>
      </div>
    </aside>
  );
}

/**
 * Returns the absolute paths of the folders that contain a workspace-relative file path.
 */
function folders(file: string): string[] {
  const segments = file.split("/").slice(0, -1);
  return segments.map((_, index) => WORKSPACE + segments.slice(0, index + 1).join("/"));
}
