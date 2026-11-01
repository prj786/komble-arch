// What a person reads for the desktop's own packages: "ewe + Word" (the
// 2026-11 revamp: lower-case ewe, a capital Word, one space), Komble and
// Shaun by their own names. Display only — packages, binaries, ids and every
// command keep their names (`pacman -S ewe-settings` still is what it is).
// Anything not listed here keeps its package name.
export const FIRST_PARTY_NAMES = {
  ewe: "ewe",
  "komble-arch": "Komble",
  "ewe-settings": "ewe Settings",
  "ewe-sync": "ewe Sync",
  "ewe-safe": "ewe Safe",
  "ewe-browser": "ewe Browser",
  "ewe-cast": "ewe Cast",
  "ewe-companion": "Shaun"
};

export const displayName = (pkg) => FIRST_PARTY_NAMES[pkg] || pkg;

// true when the name a person reads differs from the package (so a row can
// show the package as its caption)
export const renamed = (pkg) => displayName(pkg) !== pkg;
