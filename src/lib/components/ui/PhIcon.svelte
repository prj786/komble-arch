<script>
  /**
   * One glyph of the vendored Phosphor Fill font (the .ph-i face in app.css).
   * shadcn-svelte's components normally pull icons from an npm icon package;
   * they import this instead, so nothing but the font ships and the app keeps
   * a single icon language. Codepoints live in ./ph.js.
   */
  let { code, size = 14, class: className = "" } = $props();
</script>

<span
  class="ph-i shrink-0 {className}"
  style="font-size: {size}px; width: {size}px; height: {size}px"
  aria-hidden="true">{String.fromCodePoint(code)}</span
>
