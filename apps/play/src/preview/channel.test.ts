import { expect, test } from "vite-plus/test";

import { previewDocument } from "./channel.ts";

test("runtime text cannot close the preview's script element", () => {
  const document = previewDocument('const text = "</SCRIPT><script>throw 1</script>";');
  expect(document.match(/<\/script>/gi)).toHaveLength(1);
  expect(document).toContain("<\\/script>");
});
