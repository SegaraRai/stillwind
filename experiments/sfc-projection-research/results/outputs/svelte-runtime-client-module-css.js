import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";
import { css as sheet } from "stillwind/module-css";

var root = $.from_html(`<div></div>`);

export default function Svelte_module_css($$anchor, $$props) {
	$.push($$props, false);

	const props = style('sheet-style');

	$.init();

	var div = root();

	$.attribute_effect(div, () => ({ ...props }));
	$.append($$anchor, div);
	$.pop();
}