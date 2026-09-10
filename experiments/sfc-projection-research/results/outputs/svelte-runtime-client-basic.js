import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";

var root = $.from_html(`<div></div>`);

export default function Svelte_basic($$anchor, $$props) {
	$.push($$props, false);

	const props = style('script-basic');

	$.init();

	var div = root();

	$.attribute_effect(div, ($0) => ({ ...$0 }), [() => style('template-basic')]);
	$.append($$anchor, div);
	$.pop();
}