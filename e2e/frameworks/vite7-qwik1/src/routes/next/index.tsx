import { component$ } from "@builder.io/qwik";
import { Link } from "@builder.io/qwik-city";
import { style } from "stillwind/qwik";

export default component$(() => (
  <>
    <div data-testid="route-styled" {...style("p-[17px] text-[#abcdef]")}>
      Next
    </div>
    <Link data-testid="back" href="/">
      Back
    </Link>
  </>
));
