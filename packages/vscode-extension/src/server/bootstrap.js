// VS Code's Web extension host wraps classic workers to map extension resource URIs.
// Keep LSP messages until the module has installed its transport listeners.

const pending = [];

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
