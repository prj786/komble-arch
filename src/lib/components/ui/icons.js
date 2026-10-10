/**
 * Lucide codepoints, rendered through Icon.svelte and the vendored Lucide font
 * (assets/Lucide.ttf, 1.40.0) so the desktop and its apps stay one icon
 * language. Values are the font's own cmap; the shell's Theme.qml names the
 * same numbers.
 */
export const ICONS = {
  check: 0xE06C,
  caretDown: 0xE06D, // chevron-down
  caretUp: 0xE070, // chevron-up
  caretRight: 0xE06F, // chevron-right
  minus: 0xE11C,
  plus: 0xE13D,
  x: 0xE1B2,
  search: 0xE151,
  info: 0xE0F9,
  alert: 0xE077, // alert-circle
  warning: 0xE193, // alert-triangle
  success: 0xE226, // check-circle-2
  refresh: 0xE145, // refresh-cw
  rotate: 0xE149, // rotate-cw
  download: 0xE0B2,
  upload: 0xE19E,
  external: 0xE0B9, // external-link
  folder: 0xE0D7,
  folderOpen: 0xE247,
  terminal: 0xE181,
  shield: 0xE158,
  shieldAlert: 0xE1FE,
  key: 0xE4A3, // key-round
  git: 0xE0E2, // git-branch
  fileBox: 0xE310, // file-box: a package file
  fileCode: 0xE0C3, // file-code: a PKGBUILD
  copy: 0xE09E,
  trash: 0xE18E, // trash-2
  power: 0xE140,
  logOut: 0xE10E,
  arrowRight: 0xE049,
  book: 0xE05F, // book-open
  link: 0xE102,
  hardDrive: 0xE0ED,
  sparkles: 0xE412,
  packageCheck: 0xE266,
  monitorDown: 0xE421,
  history: 0xE1F5,
  eye: 0xE0BA,
  app: 0xE426, // app-window
  // the side navigation
  compass: 0xE09B,
  user: 0xE19F,
  package: 0xE129,
  puzzle: 0xE29C,
  sliders: 0xE29A, // sliders-horizontal
  settings: 0xE154
};

/**
 * The shell's Theme.qml glyph names (ewe/dotfiles/quickshell/Theme.qml) with
 * their codepoints — the same font, so a plugin manifest's `icon`
 * ("icMusic") renders here exactly as it does in the bar. A snapshot: a name
 * this table does not know falls back to the puzzle piece (themeIcon()).
 */
export const THEME_ICONS = {
  icSearch: 0xE151, icSearchOff: 0xE4AD, icClose: 0xE1B2,
  icChevronDown: 0xE06D, icChevronUp: 0xE070, icChevronRight: 0xE06F,
  icWifi: 0xE1AE, icWifiMed: 0xE5F7, icWifiLow: 0xE5F8, icWifiOff: 0xE1AF, icEthernet: 0xE620,
  icBluetooth: 0xE05C, icBluetoothOn: 0xE1B8,
  icVpn: 0xE1FF, icSsh: 0xE20A, icWeb: 0xE0E8,
  icCamera: 0xE064, icClipboard: 0xE14E, icTrash: 0xE18E, icPencil: 0xE1F9,
  icEye: 0xE0BA, icEyeOff: 0xE0BB, icDnd: 0xE11E, icCast: 0xE066,
  icSun: 0xE178, icBolt: 0xE1B4, icBattFull: 0xE055, icBattEmpty: 0xE053,
  icLeaf: 0xE2DE, icBalance: 0xE212, icSpeed: 0xE1BF,
  icVolHigh: 0xE1AB, icVolLow: 0xE1AA, icVolOff: 0xE1A9, icVolMute: 0xE1AC, icMic: 0xE118,
  icPlay: 0xE13C, icPause: 0xE12E, icPrev: 0xE15F, icNext: 0xE160, icMusic: 0xE122,
  icMonitorOff: 0xE11D, icLock: 0xE10B, icPower: 0xE140, icCog: 0xE154,
  icTiling: 0xE0FF, icFloating: 0xE426, icCheck: 0xE06C,
  icPhone: 0xE163, icMessage: 0xE116, icSend: 0xE152, icBellRing: 0xE224, icBell: 0xE059,
  icCalendar: 0xE063, icRefresh: 0xE145, icBack: 0xE048, icMail: 0xE10F,
  icFile: 0xE0C0, icFolder: 0xE0D7, icHome: 0xE0F5, icPin: 0xE259, icUser: 0xE19F, icStar: 0xE176,
  icPinOff: 0xE2B6, icGrip: 0xE0EB, icLockOpen: 0xE10C,
  icCpu: 0xE0A9, icMemory: 0xE445, icImage: 0xE0F6, icWarning: 0xE193,
  icApps: 0xE0E9, icStack: 0xE529, icPen: 0xE129, icDownload: 0xE0B2, icStore: 0xE3E4,
  icKeyboard: 0xE284, icHeadphones: 0xE0F1, icHeadset: 0xE5BD, icSpeaker: 0xE166,
  icMouse: 0xE28E, icGamepad: 0xE0DF, icTablet: 0xE17E, icLaptop: 0xE1CD, icPrinter: 0xE141,
  icCloudOk: 0xE66E, icCloudOff: 0xE08D, icCloudAlert: 0xE633, icPlus: 0xE13D,
  // names a plugin may use that Theme.qml spells differently
  icKey: 0xE4A3, icDock: 0xE529, icPlug: 0xE29C
};

/** codepoint for a Theme icon name (or one of ours); the puzzle piece otherwise */
export function themeIcon(name) {
  return THEME_ICONS[name] ?? ICONS[name] ?? ICONS.puzzle;
}
