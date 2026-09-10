import { render as $$render, createComponent as $$createComponent, maybeRenderHead as $$maybeRenderHead, spreadAttributes as $$spreadAttributes, renderScript as $$renderScript, createMetadata as $$createMetadata } from "astro/runtime/server/index.js";
import { style } from "stillwind/react";
import * as $$module1 from "stillwind/react";
export const $$metadata = $$createMetadata("astro-client-script.astro", {
	modules: [{
		module: $$module1,
		specifier: "stillwind/react",
		assert: {}
	}],
	hydratedComponents: [],
	clientOnlyComponents: [],
	hydrationDirectives: new Set([]),
	hoisted: [{
		type: "inline",
		value: `import { style as sx } from "stillwind/react";
const clientProps = sx("client");`
	}]
});
const $$AstroClientScript = $$createComponent(($$result, $$props, $$slots) => {
	const props = style("server");
	return $$render`${$$maybeRenderHead($$result)}<div${$$spreadAttributes(props)}></div>
${$$renderScript($$result, "astro-client-script.astro?astro&type=script&index=0&lang.ts")}`;
}, "astro-client-script.astro", undefined);
export default $$AstroClientScript;
