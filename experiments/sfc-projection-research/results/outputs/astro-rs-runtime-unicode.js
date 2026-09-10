import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-unicode.astro", {
	modules: [{
		module: $$module1,
		specifier: "stillwind/react",
		assert: {}
	}],
	hydratedComponents: [],
	clientOnlyComponents: [],
	hydrationDirectives: new Set([]),
	hoisted: []
});
const $$AstroUnicode = $$createComponent(($$result, $$props, $$slots) => {
	const label = "é🧭";
	const props = style("script-é🧭");
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(style("template-中🧭"))}></div>`;
}, "astro-unicode.astro", undefined);
export default $$AstroUnicode;
