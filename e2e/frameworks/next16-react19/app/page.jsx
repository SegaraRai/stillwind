import Link from "next/link";
import { style } from "stillwind/react";

import { serverLabel } from "../shared/server-only";
import Client from "./client";
import Composed from "./composed";
import Explicit from "./explicit";

export default function Page() {
  return (
    <>
      <p id="server" {...style("text-brand")}>
        {serverLabel()}
      </p>
      <Client />
      <Composed />
      <Explicit />
      <Link id="link" href="/lazy" prefetch={true}>
        Lazy
      </Link>
    </>
  );
}
