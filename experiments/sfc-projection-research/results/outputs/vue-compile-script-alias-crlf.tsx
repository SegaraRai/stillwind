import { defineComponent as _defineComponent } from 'vue'
import {
 /* alias 🧭 */ style as sx,
} from "stillwind/react";

export default /*@__PURE__*/_defineComponent({
  __name: 'vue-alias-crlf',
  setup(__props, { expose: __expose }) {
  __expose();

const props = sx('script-alias');

const __returned__ = { props, get sx() { return sx } }
Object.defineProperty(__returned__, '__isScriptSetup', { enumerable: false, value: true })
return __returned__
}

})