import { style } from "stillwind/react";

import { derived } from "../shared/derived";

export default function Composed() {
  return (
    <div id="composed" {...style(derived)}>
      composed
    </div>
  );
}
