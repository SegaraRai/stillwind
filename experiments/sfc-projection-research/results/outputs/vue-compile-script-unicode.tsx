import { defineComponent as _defineComponent } from 'vue'
import { style } from "stillwind/react";
const label="é🧭"; 
export default /*@__PURE__*/_defineComponent({
  __name: 'vue-unicode',
  setup(__props, { expose: __expose }) {
  __expose();

// 日本語 🧭
const props = style('script-é🧭');

const __returned__ = { label, props, get style() { return style } }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})