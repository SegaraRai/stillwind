import { Link } from "react-router";
import { style } from "stillwind/react";

export default function Lazy() {
  return (
    <>
      <p id="lazy" {...style("lazy")}>
        Lazy
      </p>
      <Link id="back" to="/">
        Back
      </Link>
    </>
  );
}
