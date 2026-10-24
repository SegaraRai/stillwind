import { renderToStream, type RenderToStreamOptions } from "@builder.io/qwik/server";
import { manifest } from "@qwik-client-manifest";

import Root from "./root";

export default function render(options: RenderToStreamOptions) {
  return renderToStream(<Root />, {
    manifest,
    ...options,
  });
}
