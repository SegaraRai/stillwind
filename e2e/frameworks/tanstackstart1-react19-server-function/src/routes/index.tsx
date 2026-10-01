import { createFileRoute, Link } from "@tanstack/react-router";
import { createServerFn } from "@tanstack/react-start";
import { useEffect, useState } from "react";
import { style } from "stillwind/react";

import { serverProps } from "../server-style";
import "../server-style.ts?stillwind.css";

const serverValue = createServerFn({ method: "GET" }).handler(() => ({
  props: {
    className: serverProps.className,
  },
  message: "SERVER_FUNCTION_SENTINEL",
}));

export const Route = createFileRoute("/")({
  loader: () => serverValue(),
  component: Home,
});

function Home() {
  const data = Route.useLoaderData();
  const [n, setN] = useState(0);
  useEffect(() => {
    document.documentElement.dataset.hydrated = "yes";
  }, []);

  return (
    <>
      <p id="static" {...style("brand")}>
        SSR
      </p>
      <p id="server" {...data.props}>
        Server
      </p>
      <button id="client" {...style(n > 0 ? "active" : "brand")} onClick={() => setN(n + 1)}>
        {n}
      </button>
      <button
        id="rpc"
        onClick={async () => {
          const data = await serverValue();
          document.querySelector("#server").dataset.rpc = data.message;
        }}
      >
        RPC
      </button>
      <Link id="link" to="/details" preload={false}>
        Lazy
      </Link>
    </>
  );
}
