import { createFileRoute, Link, useRouter } from "@tanstack/react-router";
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
  const router = useRouter();
  const [rpc, setRpc] = useState<typeof data>();
  const [n, setN] = useState(0);
  useEffect(() => {
    document.documentElement.dataset.hydrated = "yes";
  }, []);

  return (
    <>
      <p id="static" {...style("brand")}>
        SSR
      </p>
      <p id="server" {...(rpc ?? data).props} data-rpc={rpc?.message}>
        Server
      </p>
      <button id="client" {...style(n > 0 ? "active" : "brand")} onClick={() => setN(n + 1)}>
        {n}
      </button>
      <button
        id="rpc"
        onClick={async () => {
          setRpc(await serverValue());
        }}
      >
        RPC
      </button>
      <button id="refresh" onClick={() => router.invalidate()}>
        Refresh route data
      </button>
      <Link id="link" to="/details" preload={false}>
        Lazy
      </Link>
    </>
  );
}
