import { defineComponent as _defineComponent } from 'vue'
import { unref as _unref, normalizeProps as _normalizeProps, guardReactiveProps as _guardReactiveProps, openBlock as _openBlock, createElementBlock as _createElementBlock } from "vue"

import {
 /* alias 🧭 */ style as sx,
} from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-alias-crlf',
  setup(__props) {

const props = sx('script-alias');

return (_ctx: any,_cache: any) => {
  return (_openBlock(), _createElementBlock("div", _normalizeProps(_guardReactiveProps(_unref(sx)('template-alias'))), null, 16 /* FULL_PROPS */))
}
}

})