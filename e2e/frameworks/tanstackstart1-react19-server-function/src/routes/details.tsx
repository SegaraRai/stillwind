import { createFileRoute, Link } from "@tanstack/react-router";
import { style } from "stillwind/react";

export const Route = createFileRoute("/details")({
  component: () => (
    <>
      <p id="lazy" {...style("lazy")}>
        Lazy
      </p>
      <Link id="back" to="/">
        Back
      </Link>
    </>
  ),
});
