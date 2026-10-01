import Link from "next/link";
import { style } from "stillwind/react";

export default function Page() {
  return (
    <>
      <p id="lazy" {...style("lazy")}>
        Lazy
      </p>
      <Link id="back" href="/">
        Back
      </Link>
    </>
  );
}
