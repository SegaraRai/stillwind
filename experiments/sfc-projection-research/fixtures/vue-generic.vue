<script setup lang="ts">
import { style } from "stillwind/react";
const props = style<{label:string}>('script-generic');
</script>
<template><div v-bind="style('template-generic')"/></template>