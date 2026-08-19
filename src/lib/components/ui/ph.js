/**
 * Phosphor Fill codepoints, rendered through PhIcon.svelte + the .ph-i font
 * face so the desktop and its apps stay one icon language. The caret values
 * are verbatim from the shell's named table (hypr-shell
 * dotfiles/quickshell/Theme.qml).
 *
 * `check` deliberately differs from Theme.qml's icCheck (0xE182): in the
 * vendored Fill font 0xE182 draws check-*square* — a filled box with a
 * knocked-out tick — which reads as a second checkbox when it sits inside one.
 * 0xEBA6 is the bare tick.
 */
export const PH = {
  check: 0xeba6, // check (bare tick)
  caretDown: 0xe136, // caret-down
  caretUp: 0xe13c, // caret-up
  caretRight: 0xe13a // caret-right
};
