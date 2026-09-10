<script lang="ts">
import { style } from "stillwind/react";
const items = [{tone:'red'}];
</script>
{#each items as item}<div {...style('template-scope', item.tone)}></div>{/each}