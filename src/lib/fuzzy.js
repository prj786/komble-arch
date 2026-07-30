/** Score `q` (lowercased) against text: substring beats subsequence, 0 = no match. */
function score(q, text) {
  if (!text) return 0;
  const t = text.toLowerCase();
  const idx = t.indexOf(q);
  if (idx >= 0) return 200 - Math.min(idx, 100);
  // subsequence with a bonus for consecutive runs
  let ti = 0;
  let run = 0;
  let best = 0;
  for (const c of q) {
    const f = t.indexOf(c, ti);
    if (f < 0) return 0;
    run = f === ti ? run + 1 : 1;
    if (run > best) best = run;
    ti = f + 1;
  }
  return 20 + best;
}

/** Fuzzy search + category filter over catalog items (uses item.plainDesc). */
export function searchCatalog(items, query, category) {
  let list = category
    ? items.filter((i) => (i.categories || []).includes(category))
    : items;
  const q = (query || "").trim().toLowerCase();
  if (!q) return list;
  return list
    .map((i) => {
      const s = score(q, i.name) * 4 + score(q, i.plainDesc || "");
      return { i, s };
    })
    .filter((x) => x.s > 0)
    .sort((a, b) => b.s - a.s)
    .map((x) => x.i);
}
