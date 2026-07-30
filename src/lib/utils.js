/** Strip HTML to plain text. keepBreaks preserves paragraph/line structure. */
export function stripHtml(html, keepBreaks = false) {
  if (!html) return "";
  let src = String(html);
  if (keepBreaks) {
    src = src.replace(/<\s*(br|\/p|\/div|\/li)\s*\/?>/gi, "\n");
  }
  const doc = new DOMParser().parseFromString(src, "text/html");
  const text = doc.body.textContent || "";
  return keepBreaks
    ? text.replace(/\n{3,}/g, "\n\n").trim()
    : text.replace(/\s+/g, " ").trim();
}

/** Mirror of the backend's slug() so the UI can predict progress-event ids. */
export function slugify(s) {
  return ((s || "").toLowerCase().match(/[a-z0-9]+/g) || ["app"]).join("-");
}

export function formatBytes(n) {
  if (!n || n <= 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v >= 100 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDate(secs) {
  if (!secs) return "";
  return new Date(secs * 1000).toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric"
  });
}
