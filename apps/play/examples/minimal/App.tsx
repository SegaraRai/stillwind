import { style } from "stillwind/react";

export function App() {
  return (
    <main
      {...style("grid min-h-dvh place-items-center text-foreground bg-canvas")}
    >
      <button
        type="button"
        {...style(
          "px-4 py-2 text-action-foreground font-medium bg-action rounded-control hover:bg-action/90",
        )}
      >
        Hello, Stillwind
      </button>
    </main>
  );
}
