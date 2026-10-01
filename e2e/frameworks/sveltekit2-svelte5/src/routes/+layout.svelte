<script lang="ts">
  let { children } = $props();
</script>

<svelte:head>
  {#if import.meta.env.DEV}
    <link
      rel="stylesheet"
      href="/__stillwind/style.css?environment=ssr&module=src/routes/%2Bpage.svelte"
    />
  {/if}
</svelte:head>

{@render children()}
