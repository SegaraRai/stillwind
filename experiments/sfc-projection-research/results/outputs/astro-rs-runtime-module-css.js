import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
import * as $$module2 from "stillwind/module-css";
export const $$metadata = $$createMetadata("astro-module-css.astro", {
	modules: [{
		module: $$module1,
		specifier: "stillwind/react",
		assert: {}
	}, {
		module: $$module2,
		specifier: "stillwind/module-css",
		assert: {}
	}],
	hydratedComponents: [],
	clientOnlyComponents: [],
	hydrationDirectives: new Set([]),
	hoisted: []
});
const $$AstroModuleCss = $$createComponent(($$result, $$props, $$slots) => {
	const props = style("sheet-style");
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(props)}></div>`;
}, "astro-module-css.astro", undefined);
export default $$AstroModuleCss;
