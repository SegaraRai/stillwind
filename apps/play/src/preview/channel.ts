/**
 * Code and CSS delivered together after one complete application build.
 */
export interface PreviewUpdate {
  /** Monotonic application build sequence. Earlier messages are ignored by the frame. */
  revision: number;
  /** Bundled application code. React and React DOM use the frame's persistent runtime. */
  code: string;
  /** Complete stylesheet produced by the same compiler snapshot. */
  css: string;
}

/**
 * Creates a document for an iframe with sandbox="allow-scripts".
 *
 * @param runtime - Trusted bundled runtime. Closing script tokens are escaped for HTML parsing.
 * @returns HTML containing one persistent React mount and stylesheet element.
 */
export function previewDocument(runtime: string): string {
  return `<!doctype html><html><head><meta charset="utf-8"><style id="styles"></style></head><body><div id="root"></div><script>${runtime.replace(/<\/script/gi, "<\\/script")}</script></body></html>`;
}

/** Keeps one preview frame and sends builds over a channel that the frame claims only once. */
export class PreviewChannel {
  #port?: MessagePort;
  #update?: PreviewUpdate;

  readonly #receive: (event: MessageEvent) => void;

  /**
   * @param frame - Persistent script-only iframe.
   * @param onError - Receives frame execution errors.
   */
  constructor(frame: HTMLIFrameElement, onError: (message: string) => void) {
    this.#receive = (event) => {
      if (event.source !== frame.contentWindow || event.data !== "stillwind-preview-ready") {
        return;
      }

      this.#port?.close();

      const channel = new MessageChannel();
      this.#port = channel.port1;

      channel.port1.onmessage = ({
        data,
      }: MessageEvent<{
        error?: string;
        connected?: boolean;
      }>) => {
        if (data.error) {
          onError(data.error);
        }
        if (data.connected && this.#update) {
          channel.port1.postMessage(this.#update);
        }
      };

      frame.contentWindow?.postMessage("stillwind-preview-connect", "*", [channel.port2]);
    };

    window.addEventListener("message", this.#receive);
  }

  /**
   * Sends a completed build, or retains it until the frame connects.
   *
   * @param update - Code and CSS from one revision. Revisions must increase for subsequent builds.
   */
  update(update: PreviewUpdate): void {
    this.#update = update;
    this.#port?.postMessage(update);
  }

  /**
   * Releases the frame connection and its window listener.
   */
  dispose(): void {
    window.removeEventListener("message", this.#receive);
    this.#port?.close();
  }
}
