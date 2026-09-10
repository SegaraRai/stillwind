import { defineComponent as _defineComponent } from 'vue'
import { style } from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-generic',
  setup(__props, { expose: __expose }) {
  __expose();

const props = style<{label:string}>('script-generic');

const __returned__ = { props, get style() { return style } }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})