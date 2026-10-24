import { Link } from "gatsby";
import React from "react";
import { style } from "stillwind/react";

export default function Next() {
  return (
    <>
      <div data-testid="route-styled" {...style("p-[17px] text-[#abcdef]")}>
        Next
      </div>
      <Link data-testid="back" to="/">
        Back
      </Link>
    </>
  );
}
