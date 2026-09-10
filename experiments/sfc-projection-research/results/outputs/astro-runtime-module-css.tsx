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
import { css as sheet } from "stillwind/module-css";


import * as $$module1 from 'stillwind/react';
import * as $$module2 from 'stillwind/module-css';

export const $$metadata = $$createMetadata("astro-module-css.astro", { modules: [{ module: $$module1, specifier: 'stillwind/react', assert: {} }, { module: $$module2, specifier: 'stillwind/module-css', assert: {} }], hydratedComponents: [], clientOnlyComponents: [], hydrationDirectives: new Set([]), hoisted: [] });

const $$AstroModuleCss = $$createComponent(($$result, $$props, $$slots) => {

const props = style('sheet-style');


return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(props)}></div>`;
}, 'astro-module-css.astro', undefined);
export default $$AstroModuleCss;
