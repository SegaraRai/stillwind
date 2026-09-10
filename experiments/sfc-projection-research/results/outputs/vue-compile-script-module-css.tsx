import { defineComponent as _defineComponent } from 'vue'
import { style } from "stillwind/react";
import { css as sheet } from "stillwind/module-css";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-module-css',
  setup(__props, { expose: __expose }) {
  __expose();

const props = style('sheet-style');

const __returned__ = { props }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})