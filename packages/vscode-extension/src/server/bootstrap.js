/**
 * Loads the LSP module through VS Code's classic worker wrapper, which maps extension resource URIs.
 * The extension sends the bootstrap URI before protocol messages, which are queued during loading.
 */

const pending = [];

/**
 * Starts the module import for a bootstrap message and queues other message payloads.
 * Queued payloads are dispatched after the module installs its transport. Import failures surface
 * as worker errors on the next task.
 *
 * @param event - Worker message containing the bootstrap URI or an LSP payload.
 */
self.onmessage = (event) => {
  if (typeof event.data?.bootstrap === "string") {
    const uri = event.data.bootstrap;
    const loading = self.importExt ? self.importExt(uri) : import(uri);
    loading
      .then(() => {
        for (const data of pending) {
          self.dispatchEvent(new MessageEvent("message", { data }));
        }
        pending.length = 0;
      })
      .catch((error) => {
        setTimeout(() => {
          throw error;
        });
      });
  } else {
    pending.push(event.data);
  }
};
