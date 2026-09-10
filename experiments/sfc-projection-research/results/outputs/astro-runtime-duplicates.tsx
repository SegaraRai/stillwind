import {
  Fragment,
  render as $$render,
  createAstro as $$createAstro,
  createComponent as $$createComponent,
  renderComponent as $$renderComponent,
  renderHead as $$renderHead,
  maybeRenderHead as $$maybeRenderHead,
  unescapeHTML as $$unescapeHTML,
  renderSlot as $$renderSlot,
  mergeSlots as $$mergeSlots,
  addAttribute as $$addAttribute,
  spreadAttributes as $$spreadAttributes,
  defineStyleVars as $$defineStyleVars,
  defineScriptVars as $$defineScriptVars,
  renderTransition as $$renderTransition,
  createTransitionScope as $$createTransitionScope,
  renderScript as $$renderScript,
  createMetadata as $$createMetadata
} from "astro/runtime/server/index.js";
import { style } from "stillwind/react";


import * as $$module1 from 'stillwind/react';

export const $$metadata = $$createMetadata("astro-duplicates.astro", { modules: [{ module: $$module1, specifier: 'stillwind/react', assert: {} }], hydratedComponents: [], clientOnlyComponents: [], hydrationDirectives: new Set([]), hoisted: [] });

const $$AstroDuplicates = $$createComponent(($$result, $$props, $$slots) => {

const a = style('same');
const b = style('same');


return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(style('same'))}></div>`;
}, 'astro-duplicates.astro', undefined);
export default $$AstroDuplicates;
