# Komble (Arch)

A compact app store for **Arch Linux** — your repositories, the **AUR**, and
**AppImages**, in one window.

Built with **Tauri v2** (Rust) + **Svelte 5 + Tailwind 4**, so it ships as a
small native binary rather than a browser in a trenchcoat.

> Sibling project to [komble](https://github.com/prj786/komble) (Ubuntu/Debian).
> The AppImage half is shared; everything that touches packaging is rewritten,
> because Arch is not Debian-with-different-command-names — see below.

## Status

**Early.** The frontend builds and the Rust type-checks, but this has not yet
been run against a live pacman. Treat it as a working skeleton.

## What it does

- **Repositories** — browse and search everything in your enabled repos, filter
  by repo (core/extra/multilib/…), install and remove.
- **AUR** — search, **read the PKGBUILD**, then build. `makepkg` runs as *you*;
  only the final `pacman -U` is privileged.
- **AppImages** — the AppImageHub catalog (~1600 apps), per-user installs into
  `~/.local/share/appimages` with menu integration, no root at any point.
- **Updates** — repo, AUR and AppImage updates in one view, plus a tray
  indicator and a background check.

## Four things Arch forced, which are not stylistic choices

If you are comparing this to the Debian build, these are the real differences.

**1. There is no per-package upgrade.** Upgrading one package against a newer
sync database is a *partial upgrade*, which is unsupported on a rolling release
and the most common way to break an Arch install. The Updates view lists what is
out of date and offers exactly one action: upgrade the system. There is no
per-row Update button, and the backend has no command that could implement one.

**2. Nothing here ever runs `pacman -Sy`.** Refreshing the database without
upgrading leaves the system in precisely that partial-upgrade state. The update
list comes from `checkupdates` (pacman-contrib), which syncs into a private
temporary database and leaves the real one alone. "Refresh lists" therefore
costs nothing and touches nothing — it just drops the in-memory index.

**3. `makepkg` refuses to run as root**, so an AUR build cannot be a privileged
verb like every other action. Komble clones into its cache, builds as the
invoking user with `PACMAN_AUTH=pkexec` (so makepkg's own dependency installs
elevate properly instead of hanging on a `sudo` prompt with no terminal), and
then hands the built `*.pkg.tar.zst` to the privileged helper.

**4. A PKGBUILD is a shell script that runs with your privileges.** So the review
step is mandatory, not a setting: Komble fetches and displays the PKGBUILD, and
the build button does not exist until you have it on screen. That is the whole
security model for the AUR path.

## Privilege model

One polkit action, authorising one **path**:

```
/usr/lib/komble/komble-helper      ← polkit authorises this
```

The helper's `case` statement is therefore the complete definition of what
Komble may do as root: `install-repo`, `install-file`, `remove`, `sysupgrade`.
Every branch `exec`s, every branch puts `--` before user data so a package named
`-Qi` cannot become a flag, and arity is checked before validation. Package
names and file paths are validated in Rust *and* again in the helper.

Nothing is ever passed through a shell. Commands are built as argv vectors, so
there is no interpolation and no metacharacter surface.

Without the helper installed (e.g. `npm run tauri dev`), Komble falls back to
`pkexec pacman …` with a fixed argv — correct, but one auth prompt per action
instead of one per session.

## Build

```bash
npm install
npm run tauri dev        # development
npm run tauri build      # release binary
```

Build dependencies: `rust`, `nodejs`, `npm`, `webkit2gtk-4.1`, `gtk3`,
`libayatana-appindicator`.

## Install

Tauri v2 has no pacman bundler, so the `PKGBUILD` is the packaging path — it
also installs the polkit policy and the helper, which the Debian build carried
inside its `.deb`.

```bash
makepkg -si
```

Runtime optional dependencies, all degraded gracefully:

| Package | Without it |
|---|---|
| `pacman-contrib` | no update checking (`checkupdates` is the only safe way to ask) |
| `expac` | package list still works, but with no descriptions |
| `base-devel`, `git` | AUR builds unavailable |
| `fuse2` | AppImages integrate but will not launch |

## Layout

```
src/                  Svelte frontend (views in src/lib/components)
src-tauri/src/        lib.rs (tray, setup), pacman.rs (repos + AUR),
                      catalog.rs + appimage.rs (shared with the Debian build),
                      system.rs, registry.rs, util.rs
packaging/            komble-helper, polkit policy, .desktop
PKGBUILD              how this is actually installed
```

## Licence

MIT.
