import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style as sx } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-alias-crlf.astro", {
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
const $$AstroAliasCrlf = $$createComponent(($$result, $$props, $$slots) => {
	const props = sx("script-alias");
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(sx("template-alias"))}></div>`;
}, "astro-alias-crlf.astro", undefined);
export default $$AstroAliasCrlf;
