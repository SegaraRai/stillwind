import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";
import { css as sheet } from "stillwind/module-css";

export default function Svelte_module_css($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const props = style('sheet-style');

		$$renderer.push(`<div${$.attributes({ ...props })}></div>`);
	});
}