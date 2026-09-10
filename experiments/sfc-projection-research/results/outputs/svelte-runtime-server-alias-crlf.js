import * as $ from 'svelte/internal/server';
import { style as sx } from "stillwind/react";

export default function Svelte_alias_crlf($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		/* alias 🧭 */
		const props = sx('script-alias');

		$$renderer.push(`<div${$.attributes({ ...sx('template-alias') })}></div>`);
	});
}