import * as $ from 'svelte/internal/server';
import { style } from "stillwind/react";
import { style as sx } from "stillwind/react";

export const moduleProps = sx('module');

export default function Svelte_two_scripts($$renderer, $$props) {
	$$renderer.component(($$renderer) => {
		const props = style('instance');

		$$renderer.push(`<div${$.attributes({ ...props })}></div>`);
	});
}