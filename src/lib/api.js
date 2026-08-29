import { invoke } from "@tauri-apps/api/core";

// catalog
export const fetchCatalog = (force = false) => invoke("fetch_catalog", { force });
export const resolveRelease = (githubUrl, token) =>
  invoke("resolve_release", { githubUrl, token: token || null });
// AM catalog items carry no download URL — this reads the app's AM install
// recipe and returns { github, release } (github set when the app lives there).
export const resolveAmApp = (id, token) =>
  invoke("resolve_am_app", { id, token: token || null });

// appimages
export const installAppimage = (req) => invoke("install_appimage", { req });
export const removeAppimage = (id) => invoke("remove_appimage", { id });
export const listAppimages = () => invoke("list_appimages");
export const checkAppimageUpdates = (token) =>
  invoke("check_appimage_updates", { token: token || null });
export const updateAppimage = (id, token) =>
  invoke("update_appimage", { id, token: token || null });
export const appimageFileInfo = (path) => invoke("appimage_file_info", { path });
export const installLocalAppimage = (path, name, version, dir) =>
  invoke("install_local_appimage", {
    path,
    name,
    version: version || null,
    dir: dir || null
  });

// packages (pacman repos + AUR)
export const packageFileInfo = (path) => invoke("package_file_info", { path });
export const installPackageFile = (path) => invoke("install_package_file", { path });
export const removePackage = (pkg) => invoke("remove_package", { package: pkg });
export const listTrackedPackages = () => invoke("list_tracked_packages");
export const listUpgradable = () => invoke("list_upgradable");
// "Refresh lists" is free and touches nothing: checkupdates works against its
// own database copy, so there is no `pacman -Sy` to run (and running one would
// arm a partial upgrade).
export const refreshLists = () => invoke("refresh_lists");
// There is no per-REPO-package upgrade on purpose — a partial upgrade is
// unsupported on a rolling release. The only repo upgrade is the whole system.
export const systemUpgrade = () => invoke("system_upgrade");
// -Syu never touches foreign packages: AUR ones are rebuilt separately, each
// through the same clone → makepkg → pacman -U pipeline as an install.
export const aurUpgrade = () => invoke("aur_upgrade");

// first-party ewe apps (Komble, ewe-settings) + the desktop itself
export const firstPartyStatus = (token) =>
  invoke("first_party_status", { token: token || null });
export const installFirstParty = (pkg, token) =>
  invoke("install_first_party", { pkg, token: token || null });
export const eweStatus = (token) => invoke("ewe_status", { token: token || null });
export const eweUpdate = () => invoke("ewe_update");
export const eweUpdateTerminal = () => invoke("ewe_update_terminal");
export const takePendingRoute = () => invoke("take_pending_route");
export const browsePackages = (query, section, limit) =>
  invoke("browse_packages", {
    query: query || null,
    section: section || null,
    limit: limit || null
  });
export const packageRepos = () => invoke("package_repos");
export const packageInfo = (pkg) => invoke("package_info", { package: pkg });
export const installPackage = (pkg) => invoke("install_package", { package: pkg });

// AUR — aurPkgbuild is a mandatory review step, not an optional one: a PKGBUILD
// is a shell script that runs with your privileges at build time.
export const aurSearch = (query) => invoke("aur_search", { query });
export const aurPkgbuild = (pkg) => invoke("aur_pkgbuild", { package: pkg });
export const aurInstall = (pkg) => invoke("aur_install", { package: pkg });

// ewe integration (accent/look + the shell-owned Google account)
export const dePrefs = () => invoke("de_prefs");
export const qsIpc = (func) => invoke("qs_ipc", { func });

// system
export const systemCheck = () => invoke("system_check");
export const installFuse2 = () => invoke("install_fuse2");
export const installPacmanContrib = () => invoke("install_pacman_contrib");
export const checkSelfUpdate = () => invoke("check_self_update");
// file the app was opened with (double-click on an .AppImage / package file)
export const takePendingOpen = () => invoke("take_pending_open");

/**
 * Install a catalog item. GitHub-backed apps resolve the real .AppImage asset
 * via the Releases API; AM catalog items resolve through their AM install
 * recipe (which is GitHub for most of them — then updates are tracked too).
 */
export async function installFromItem(item, settings) {
  const base = {
    id: item.id,
    name: item.name,
    iconUrl: item.icon || null,
    dir: settings?.appimageDir || null
  };
  if (item.github) {
    const rel = await resolveRelease(item.github, settings?.githubToken);
    return installAppimage({
      ...base,
      url: rel.assets[0].url,
      version: rel.version,
      repo: item.github
    });
  }
  if (item.source === "am") {
    const r = await resolveAmApp(item.id, settings?.githubToken);
    return installAppimage({
      ...base,
      url: r.release.assets[0].url,
      version: r.release.version,
      repo: r.github
    });
  }
  if (item.download && /\.appimage$/i.test(item.download.split(/[?#]/)[0])) {
    return installAppimage({ ...base, url: item.download, version: null, repo: null });
  }
  throw new Error(
    "No installable release found for this app. Try its download page instead."
  );
}

// RFC-002: the one-file restore surface (manifest read through ewe-conf) +
// push/pull of ewe.conf itself
export const restoreManifest = () => invoke("restore_manifest");
export const confSync = (direction) => invoke("conf_sync", { direction });
