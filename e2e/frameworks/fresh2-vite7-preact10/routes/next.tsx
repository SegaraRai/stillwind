import { style } from "stillwind/preact";

export default function Next() {
  return (
    <>
      <div data-testid="route-styled" {...style("p-[17px] text-[#abcdef]")}>
        Next
      </div>
      <a data-testid="back" href="/">
        Back
      </a>
    </>
  );
}
