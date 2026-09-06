import { style } from "stillwind/react";

export function RelationshipCard() {
  return (
    <section {...style("group/menu container @container/sidebar rounded-lg p-4")}>
      <input type="checkbox" {...style("peer/field")} />
      <h2
        {...style(
          "group-hover/menu:text-brand peer-checked/field:underline @sm:grid @max-md:hidden",
        )}
      >
        Relationship compatibility
      </h2>
    </section>
  );
}
