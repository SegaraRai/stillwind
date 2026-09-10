import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import { style as sx } from "stillwind/react";
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";

export const moduleProps = sx('module');

var root = $.from_html(`<div></div>`);

export default function Svelte_two_scripts($$anchor, $$props) {
	$.push($$props, false);

	const props = style('instance');

	$.init();

	var div = root();

	$.attribute_effect(div, () => ({ ...props }));
	$.append($$anchor, div);
	$.pop();
}