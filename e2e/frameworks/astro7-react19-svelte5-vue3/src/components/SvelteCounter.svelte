<script module lang="ts">
  import { css } from "stillwind/module-css";

  export { css };
</script>

<script lang="ts">
  import { style } from "stillwind/svelte";
  import { onMount } from "svelte";

  import { probeComposed, probeDiamond } from "./semantics-svelte";

  let count = $state(0);
  let phase = $state(0);

  onMount(() => {
    document.documentElement.dataset.svelteReady = "yes";
  });
</script>

<button {...style("text-probe")} data-testid="svelte-counter" onclick={() => count++}
  >{count}</button
>

<div data-testid="svelte-composed" {...style(probeComposed)}>Composition</div>
<div data-testid="svelte-diamond" {...style(probeDiamond)}>Diamond</div>
<div
  data-testid="svelte-conditional"
  {...style("m-[40px]", phase > 0 && (phase > 1 ? "m-[16px]" : "m-[24px]"), {
    "aria-busy": String(phase > 0),
  })}
>
  Conditional
</div>
<div data-testid="svelte-conditional-active" {...style("m-[40px]", phase >= 0 && "m-[16px]")}>
  Active
</div>
<button data-testid="svelte-toggle" onclick={() => (phase = (phase + 1) % 3)}>Condition</button>
