import { useState } from "react";
import { style } from "stillwind/react";

import { control, panel, toggle } from "./styles";
import { initialTasks, type Task } from "./tasks";
import { ThemeSwitch, type Theme } from "./ThemeSwitch";

// A ref names each task row. Its children react to the row's state and width.
const row = style.ref("row");

export function App() {
  const [theme, setTheme] = useState<Theme>("system");
  const [tasks, setTasks] = useState(initialTasks);
  const [openOnly, setOpenOnly] = useState(false);

  const done = tasks.filter((task) => task.done).length;
  const visible = openOnly ? tasks.filter((task) => !task.done) : tasks;

  const update = (id: number, change: Partial<Task>) =>
    setTasks(
      tasks.map((task) => (task.id === id ? { ...task, ...change } : task)),
    );

  return (
    <main
      data-theme={theme}
      {...style(
        "min-h-dvh p-gutter text-foreground bg-canvas scheme-light-dark",
        {
          "data-[theme=light]:": "scheme-light",
          "data-[theme=dark]:": "scheme-dark",
        },
      )}
    >
      <div
        {...style(
          "grid mx-auto max-w-page gap-gutter sidebar:grid-cols-[minmax(0,1fr)_var(--container-summary)]",
        )}
      >
        <header
          {...style(
            "flex flex-wrap items-end justify-between gap-4 sidebar:col-span-2",
          )}
        >
          <div>
            <p {...style("text-muted text-sm")}>Release 2.4</p>
            <h1 {...style("text-title font-semibold")}>Release checklist</h1>
          </div>
          <ThemeSwitch value={theme} onChange={setTheme} />
        </header>

        <section aria-label="Tasks" {...style(panel, "grid gap-4")}>
          <div role="group" aria-label="Filter" {...style("flex gap-1")}>
            <button
              type="button"
              aria-pressed={!openOnly}
              onClick={() => setOpenOnly(false)}
              {...toggle}
            >
              All
            </button>
            <button
              type="button"
              aria-pressed={openOnly}
              onClick={() => setOpenOnly(true)}
              {...toggle}
            >
              Open
            </button>
          </div>
          <ul {...style("grid gap-2")}>
            {visible.map((task) => (
              <li
                key={task.id}
                data-done={task.done || undefined}
                {...style(
                  row,
                  row.asContainer("inline-size"),
                  "flex p-3 items-center gap-3 rounded-control border border-border",
                )}
              >
                <button
                  type="button"
                  role="checkbox"
                  aria-checked={task.done}
                  aria-label={task.title}
                  onClick={() => update(task.id, { done: !task.done })}
                  {...style(
                    "grid size-7 shrink-0 place-items-center text-action-foreground rounded-full border-2 border-border",
                    "focus-visible:focus-ring aria-checked:bg-action aria-checked:border-action hocus:border-action",
                  )}
                >
                  <span
                    aria-hidden="true"
                    {...style("icon-control icon-lucide--check invisible", {
                      [row("data-done")]: "visible",
                    })}
                  />
                </button>
                {/* Owner and priority wrap below the title in a narrow row. */}
                <div
                  {...style("grid min-w-0 flex-1 gap-1", {
                    [row.at("width >= 28rem")]:
                      "flex items-center justify-between",
                  })}
                >
                  <p
                    {...style("truncate", {
                      [row("data-done")]: "text-muted line-through",
                    })}
                  >
                    {task.title}
                  </p>
                  <p {...style("flex items-center gap-2 text-muted text-sm")}>
                    {task.owner}
                    <span
                      data-priority={task.priority}
                      {...style(
                        "px-2 text-xs font-medium rounded-full badge-muted",
                        {
                          "data-[priority=high]:": "badge-danger",
                          "data-[priority=medium]:": "badge-warning",
                        },
                      )}
                    >
                      {task.priority}
                    </span>
                  </p>
                </div>
                {/* The remove button appears while the row is hovered or the button is focused. */}
                <button
                  type="button"
                  aria-label={`Remove ${task.title}`}
                  onClick={() =>
                    setTasks(tasks.filter((other) => other.id !== task.id))
                  }
                  {...style(
                    control,
                    "text-muted opacity-0 focus-visible:opacity-100",
                    {
                      [row("hover")]: "opacity-100",
                    },
                  )}
                >
                  <span
                    aria-hidden="true"
                    {...style("icon-control icon-lucide--trash-2")}
                  />
                </button>
              </li>
            ))}
          </ul>
        </section>

        <aside aria-label="Progress" {...style(panel, "grid h-fit gap-3")}>
          <p {...style("flex items-baseline justify-between")}>
            <span {...style("font-medium")}>Progress</span>
            <span {...style("text-muted text-sm")}>
              {done} of {tasks.length} done
            </span>
          </p>
          <div {...style("overflow-hidden h-2 bg-surface-muted rounded-full")}>
            {/* The width is a runtime value, so the utility reads it from a custom property. */}
            <div
              {...style(
                "h-full w-(--progress) bg-success rounded-full transition-[width]",
                {
                  "--progress": `${tasks.length ? (done / tasks.length) * 100 : 0}%`,
                },
              )}
            />
          </div>
        </aside>
      </div>
    </main>
  );
}
