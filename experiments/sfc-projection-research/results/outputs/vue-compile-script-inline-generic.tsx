import { defineComponent as _defineComponent } from 'vue'
import { unref as _unref, normalizeProps as _normalizeProps, guardReactiveProps as _guardReactiveProps, openBlock as _openBlock, createElementBlock as _createElementBlock } from "vue"

import { style } from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-generic',
  setup(__props) {

const props = style<{label:string}>('script-generic');

return (_ctx: any,_cache: any) => {
  return (_openBlock(), _createElementBlock("div", _normalizeProps(_guardReactiveProps(_unref(style)('template-generic'))), null, 16 /* FULL_PROPS */))
}
}

})