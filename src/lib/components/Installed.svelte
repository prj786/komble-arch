<script>
  import { installed, trackedPkgs, firstPartyPkgs, progress, route, toast } from "../stores";
  import { refreshInstalled, refreshPkgs } from "../actions";
  import * as api from "../api";
  import { formatDate } from "../utils";
  import { displayName } from "../names.js";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Icon from "./ui/Icon.svelte";
  import AppIcon from "./AppIcon.svelte";

  // One row anatomy (List row), the same as Plugins: icon, name, one line,
  // the source as a tag, the action last. Remove is ghost with the trash
  // glyph and takes two presses: the first arms it (4 s), the second removes.
  let confirming = null;

  function askConfirm(key) {
    confirming = key;
    setTimeout(() => {
      if (confirming === key) confirming = null;
    }, 4000);
  }

  async function removeAppimage(entry) {
    if (confirming !== `ai:${entry.id}`) return askConfirm(`ai:${entry.id}`);
    confirming = null;
    try {
      await api.removeAppimage(entry.id);
      toast(`Removed **${entry.name}**`, "success");
      refreshInstalled();
    } catch (e) {
      toast(e, "error");
    }
  }

  async function removeDebPkg(d) {
    if (confirming !== `pkg:${d.package}`) return askConfirm(`pkg:${d.package}`);
    confirming = null;
    try {
      toast(`Removing **${d.package}**. You may be asked for your password…`, "info");
      await api.removePackage(d.package);
      toast(`Removed **${d.package}**`, "success");
      refreshPkgs();
    } catch (e) {
      toast(e, "error");
    }
  }

  $: nothing = !$firstPartyPkgs.length && !$installed.length && !$trackedPkgs.length;
  const line = (...parts) => parts.filter(Boolean).join(" · ");
</script>

<Page title="Installed" desc="Apps Komble manages on this machine">
  {#if nothing}
    <!-- one empty state for the page, not one per group -->
    <div class="ewe-empty">
      <span class="ewe-empty__icon"><Icon name="package" /></span>
      <div class="ewe-empty__title">Nothing installed with Komble yet</div>
      <div class="ewe-empty__desc">Apps you install from Discover, the AUR or an AppImage file show up here.</div>
      <div class="ewe-empty__actions">
        <button class="ewe-btn ewe-btn--primary" on:click={() => route.set("discover")}><Icon name="compass" />Discover apps</button>
      </div>
    </div>
  {:else}
    <!-- The desktop's own apps arrive as dependencies of the ewe package, not
         through Komble, so they were missing here while Updates listed them.
         No Remove: pacman would refuse while ewe depends on them. -->
    {#if $firstPartyPkgs.length}
      <Group title="ewe apps · {$firstPartyPkgs.length}">
        {#each $firstPartyPkgs as f (f.pkg)}
          <div class="ewe-row ewe-row--tall" title="{f.pkg}: comes with ewe and updates with the desktop">
            <span class="ewe-row__lead"><AppIcon name={displayName(f.pkg)} pkg size="sm" /></span>
            <div class="ewe-row__text">
              <div class="ewe-row__title">{displayName(f.pkg)}</div>
              <div class="ewe-row__desc">{f.summary}</div>
            </div>
            <div class="ewe-row__trail">
              <span class="ver">{f.version}</span>
            </div>
          </div>
        {/each}
      </Group>
    {/if}

    <Group title="AppImages · {$installed.length}">
      {#if $installed.length === 0}
        <div class="ewe-empty ewe-empty--compact">
          <span class="ewe-empty__icon"><Icon name="app" /></span>
          <div class="ewe-empty__title">No AppImages yet</div>
          <div class="ewe-empty__desc">Install one from Discover, or drop an .AppImage file on this window.</div>
          <div class="ewe-empty__actions">
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => route.set("discover")}>Discover apps</button>
          </div>
        </div>
      {:else}
        {#each $installed as entry (entry.id)}
          <div class="ewe-row ewe-row--tall">
            <span class="ewe-row__lead"><AppIcon name={entry.name} src={entry.iconUrl} size="sm" /></span>
            <div class="ewe-row__text">
              <div class="ewe-row__title">{entry.name}</div>
              <div class="ewe-row__desc" title={entry.path}>{line(entry.version, formatDate(entry.installedAt), entry.path)}</div>
            </div>
            <div class="ewe-row__trail">
              <span class="ewe-tag">AppImage</span>
              {#if $progress[entry.id]}
                <span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Updating…</span>
              {:else}
                <button
                  class="ewe-btn ewe-btn--sm {confirming === `ai:${entry.id}` ? 'ewe-btn--danger' : 'ewe-btn--ghost is-danger'}"
                  aria-label={confirming === `ai:${entry.id}` ? undefined : `Remove ${entry.name}`}
                  on:click={() => removeAppimage(entry)}
                >
                  <Icon name="trash" />{confirming === `ai:${entry.id}` ? `Remove ${entry.name}` : "Remove"}
                </button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </Group>

    <Group title="Packages installed with Komble · {$trackedPkgs.length}">
      {#if $trackedPkgs.length === 0}
        <div class="ewe-empty ewe-empty--compact">
          <span class="ewe-empty__icon"><Icon name="package" /></span>
          <div class="ewe-empty__title">No packages yet</div>
          <div class="ewe-empty__desc">Packages you install from the repositories or the AUR show up here.</div>
          <div class="ewe-empty__actions">
            <button class="ewe-btn ewe-btn--sm ewe-btn--secondary" on:click={() => route.set("discover")}>Discover apps</button>
          </div>
        </div>
      {:else}
        {#each $trackedPkgs as d (d.package)}
          <div class="ewe-row ewe-row--tall">
            <span class="ewe-row__lead"><AppIcon name={d.package} pkg size="sm" /></span>
            <div class="ewe-row__text">
              <div class="ewe-row__title">{d.package}</div>
              <div class="ewe-row__desc">{line(d.version, d.description, formatDate(d.installedAt))}</div>
            </div>
            <div class="ewe-row__trail">
              {#if d.source === "aur"}
                <span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">AUR</span></span>
              {:else}
                <span class="ewe-tag">{d.source === "repo" || !d.source ? "Repository" : d.source}</span>
              {/if}
              <button
                class="ewe-btn ewe-btn--sm {confirming === `pkg:${d.package}` ? 'ewe-btn--danger' : 'ewe-btn--ghost is-danger'}"
                aria-label={confirming === `pkg:${d.package}` ? undefined : `Remove ${d.package}`}
                on:click={() => removeDebPkg(d)}
              >
                <Icon name="trash" />{confirming === `pkg:${d.package}` ? `Remove ${d.package}` : "Remove"}
              </button>
            </div>
          </div>
        {/each}
      {/if}
    </Group>
  {/if}
</Page>
