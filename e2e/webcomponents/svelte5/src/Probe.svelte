<svelte:options customElement="probe-card" />

<script lang="ts">
  import { styleSheets } from "stillwind/css";
  import { parts } from "stillwind/module-css";
  import { style } from "stillwind/svelte";
  import { onMount } from "svelte";

  import { shared } from "./shared-style";

  let { tone = "red" } = $props<{ tone?: string }>();
  let count = $state(0);
  const host = $host();

  onMount(() => {
    host.shadowRoot!.adoptedStyleSheets = styleSheets(parts, host.ownerDocument);
  });
</script>

<div data-testid="styled" {...style(shared, "text-probe p-[12px]")}>Styled</div>
<div data-testid="dynamic" {...style("text-[color:var(--tone)]", { "--tone": tone })}>Dynamic</div>
<button
  data-testid="dynamic-toggle"
  onclick={() => {
    tone = "blue";
  }}>Tone</button
>
<button data-testid="counter" onclick={() => count++}>{count}</button>
<div data-testid="authored" class="authored">Authored</div>
