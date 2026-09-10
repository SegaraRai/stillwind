import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";

export default function Svelte_generic($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const props = style('script-generic');

		$$renderer.push(`<div${$.attributes({ ...style('template-generic') })}></div>`);
	});
}