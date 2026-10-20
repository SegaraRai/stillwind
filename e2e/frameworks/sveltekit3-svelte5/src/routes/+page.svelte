<script lang="ts">
  import { style } from "stillwind/svelte";
  import { onMount } from "svelte";

  import { probeComposed, probeDiamond } from "../semantics-main";
  import { shared } from "../shared-style";

  let count = $state(0);
  let phase = $state(0);

  onMount(() => {
    document.documentElement.dataset.hydrated = "yes";
  });
</script>

<main>
  <div data-testid="styled" {...style(shared, "text-probe p-[12px]")}>Styled</div>
  <button data-testid="counter" onclick={() => count++}>{count}</button>
</main>

<div data-testid="main-composed" {...style(probeComposed)}>Composition</div>
<div data-testid="main-diamond" {...style(probeDiamond)}>Diamond</div>
<div
  data-testid="main-conditional"
  aria-busy={phase > 0 ? "true" : "false"}
  {...style("m-[40px]", phase > 0 && (phase > 1 ? "m-[16px]" : "m-[24px]"))}
>
  Conditional
</div>
<div data-testid="main-conditional-active" {...style("m-[40px]", phase >= 0 && "m-[16px]")}>
  Active
</div>
<button data-testid="main-toggle" onclick={() => (phase = (phase + 1) % 3)}>Condition</button>
