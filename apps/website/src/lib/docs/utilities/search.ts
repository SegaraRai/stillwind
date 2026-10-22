export interface UtilityRecord {
  name: string;
  css: string;
  marker: boolean;
}

export interface UtilityCatalog {
  version: string;
  revision: string;
  records: UtilityRecord[];
}

export function searchUtilities(
  records: readonly UtilityRecord[],
  mode: "utility" | "css",
  query: string,
  limit = 60,
) {
  const value = query.trim().toLowerCase();
  const terms = value.split(/[\s:;{}]+/).filter(Boolean);
  const matches = records.filter((record) => {
    const text = (mode === "utility" ? record.name : record.css).toLowerCase();
    return terms.every((term) => text.includes(term));
  });
  if (mode === "utility" && value) {
    matches.sort(
      (a, b) =>
        Number(b.name === value) - Number(a.name === value) ||
        Number(b.name.startsWith(value)) - Number(a.name.startsWith(value)),
    );
  }
  return { total: matches.length, records: matches.slice(0, limit) };
}
