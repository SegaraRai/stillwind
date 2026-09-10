<script lang="ts">
import { style as sx } from "stillwind/react";
export const shared=sx('normal');
export default {name:'Two'};
</script><script setup lang="ts">
import { style } from "stillwind/react";
const props=style('setup');
</script>
<template><div v-bind="props"/></template>