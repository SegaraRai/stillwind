import 'svelte/internal/disclose-version';
import 'svelte/internal/flags/legacy';
import * as $ from 'svelte/internal/client';
import { style } from "stillwind/react";

var root = $.from_html(`<div></div>`);

export default function Svelte_unicode($$anchor, $$props) {
	$.push($$props, false);

	// 日本語 🧭
	const label = "é🧭";

	const props = style('script-é🧭');

	$.init();

	var div = root();

	$.attribute_effect(div, ($0) => ({ ...$0 }), [() => style('template-中🧭')]);
	$.append($$anchor, div);
	$.pop();
}