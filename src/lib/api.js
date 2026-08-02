import { invoke } from "@tauri-apps/api/core";

// catalog
export const fetchCatalog = (force = false) => invoke("fetch_catalog", { force });
export const resolveRelease = (githubUrl, token) =>
  invoke("resolve_release", { githubUrl, token: token || null });

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
// There is no per-package upgrade on purpose — a partial upgrade is unsupported
// on a rolling release. The only upgrade is the whole system.
export const systemUpgrade = () => invoke("system_upgrade");
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

// hypr-shell integration (accent/look + the shell-owned Google account)
export const dePrefs = () => invoke("de_prefs");
export const qsIpc = (func) => invoke("qs_ipc", { func });
export const backupPackages = () => invoke("backup_packages");

// system
export const systemCheck = () => invoke("system_check");
export const installFuse2 = () => invoke("install_fuse2");
export const checkSelfUpdate = () => invoke("check_self_update");

/**
 * Install a catalog item: resolve the real .AppImage asset via the GitHub
 * Releases API (feed Download links are often null or point at release pages),
 * falling back to a direct Download link when it already ends in .AppImage.
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
  if (item.download && /\.appimage$/i.test(item.download.split(/[?#]/)[0])) {
    return installAppimage({ ...base, url: item.download, version: null, repo: null });
  }
  throw new Error(
    "No installable release found for this app. Try its download page instead."
  );
}
