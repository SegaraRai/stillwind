import { style } from "stillwind/react";

export function Colors() {
  return (
    <>
      <div data-case="theme" {...style("bg-red-500/50")} />
      <div data-case="zero" {...style("bg-red-500/0")} />
      <div data-case="opaque" {...style("bg-red-500/100")} />
      <div data-case="literal" {...style("bg-[red]/50")} />
      <div data-case="transparent" {...style("bg-transparent/50")} />
      <div data-case="current" {...style("bg-current/50")} />
      <div data-case="variable" {...style("bg-(--brand)/50")} />
      <div data-case="opacity-variable" {...style("bg-[red]/(--opacity)")} />
      <div data-case="theme-opacity-variable" {...style("bg-red-500/(--opacity)")} />
      <div data-case="alias" {...style("bg-alias/50")} />
      <div data-case="linked" {...style("bg-linked/50")} />
      <div data-case="nested-theme" {...style("bg-nested/50")} />
      <div data-case="shadow-theme" {...style("shadow-red-500/50")} />
      <div data-case="shadow-literal" {...style("shadow-[color:red]/50")} />
      <div data-case="shadow-variable" {...style("shadow-(color:--brand)/50")} />
      <div data-case="ring" {...style("ring-red-500/50")} />
      <div data-case="gradient" {...style("from-(--brand)/50")} />
      <div
        data-case="arbitrary"
        {...style("[color:color-mix(in_oklab,var(--brand)_50%,transparent)]")}
      />
    </>
  );
}
