import { style as astro } from "stillwind/astro";
import css from "stillwind/module-css";
import { style as preact } from "stillwind/preact";
import { style as react } from "stillwind/react";
import { style as solid } from "stillwind/solid";
import { style as svelte } from "stillwind/svelte";
import { style as vue } from "stillwind/vue";

type Assert<T extends true> = T;
type ClassProps<T> = "class" extends keyof T ? ("className" extends keyof T ? false : true) : false;
export type FrameworkSpreadContracts = [
  Assert<ClassProps<ReturnType<typeof astro>>>,
  Assert<ClassProps<ReturnType<typeof preact>>>,
  Assert<ClassProps<ReturnType<typeof solid>>>,
  Assert<ClassProps<ReturnType<typeof svelte>>>,
  Assert<ClassProps<ReturnType<typeof vue>>>,
  Assert<"className" extends keyof ReturnType<typeof react> ? true : false>,
  Assert<typeof css extends string ? true : false>,
];
