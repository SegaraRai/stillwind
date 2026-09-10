import { defineComponent as _defineComponent } from 'vue'
import { unref as _unref, normalizeProps as _normalizeProps, guardReactiveProps as _guardReactiveProps, openBlock as _openBlock, createElementBlock as _createElementBlock } from "vue"

import { style } from "stillwind/react";
const label="é🧭"; 
export default /*@__PURE__*/_defineComponent({
  __name: 'vue-unicode',
  setup(__props) {

// 日本語 🧭
const props = style('script-é🧭');

return (_ctx: any,_cache: any) => {
  return (_openBlock(), _createElementBlock("div", _normalizeProps(_guardReactiveProps(_unref(style)('template-中🧭'))), null, 16 /* FULL_PROPS */))
}
}

})