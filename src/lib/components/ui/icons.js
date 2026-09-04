/**
 * Lucide codepoints, rendered through Icon.svelte + the .icon font face so
 * the desktop and its apps stay one icon language. Values are verbatim from
 * the vendored font's own font/codepoints.json — the same numbers the shell's
 * named table in dotfiles/quickshell/Theme.qml uses.
 */
export const ICONS = {
  check: 0xE06C, // check
  caretDown: 0xE06D, // chevron-down
  caretUp: 0xE070, // chevron-up
  caretRight: 0xE06F, // chevron-right
  minus: 0xE11C // minus — Lucide has one, so the indeterminate checkbox
                 // no longer needs the hand-drawn bar it used to fall back to
};
