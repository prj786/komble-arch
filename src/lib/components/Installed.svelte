<script>
  import { installed, trackedPkgs, progress, toast } from "../stores";
  import { refreshInstalled, refreshPkgs } from "../actions";
  import * as api from "../api";
  import { formatDate } from "../utils";
  import Page from "./ui/Page.svelte";
  import Group from "./ui/Group.svelte";
  import Icon from "./ui/Icon.svelte";
  import AppIcon from "./AppIcon.svelte";

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
</script>

<Page title="Installed" desc="Apps Komble manages on this machine">
  <Group title="AppImages · {$installed.length}">
    {#if $installed.length === 0}
      <div class="ewe-empty ewe-empty--compact">
        <span class="ewe-empty__icon"><Icon name="app" /></span>
        <div class="ewe-empty__title">No AppImages yet</div>
        <div class="ewe-empty__desc">Install one from Discover.</div>
      </div>
    {:else}
      {#each $installed as entry (entry.id)}
        <div class="ewe-row ewe-row--tall">
          <span class="ewe-row__lead"><AppIcon name={entry.name} src={entry.iconUrl} size="sm" /></span>
          <div class="ewe-row__text">
            <div class="row-title">
              <span class="ewe-row__title">{entry.name}</span>
              <span class="ewe-badge"><span class="ewe-badge__label">{entry.version}</span></span>
            </div>
            <div class="ewe-row__desc" title={entry.path}>{entry.path} · {formatDate(entry.installedAt)}</div>
          </div>
          <div class="ewe-row__trail">
            {#if $progress[entry.id]}
              <span class="busy"><span class="ewe-spinner ewe-spinner--sm" aria-hidden="true"></span>Updating…</span>
            {:else}
              <!-- two steps: the first press arms it (4 s), the second removes -->
              <button
                class="ewe-btn ewe-btn--sm {confirming === `ai:${entry.id}` ? 'ewe-btn--danger' : 'ewe-btn--ghost'}"
                on:click={() => removeAppimage(entry)}
              >
                {confirming === `ai:${entry.id}` ? `Remove ${entry.name}` : "Remove"}
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
        <div class="ewe-empty__desc">Packages you install with Komble appear here.</div>
      </div>
    {:else}
      {#each $trackedPkgs as d (d.package)}
        <div class="ewe-row ewe-row--tall">
          <span class="ewe-row__lead"><AppIcon name={d.package} pkg size="sm" /></span>
          <div class="ewe-row__text">
            <div class="row-title">
              <span class="ewe-row__title">{d.package}</span>
              <span class="ewe-badge"><span class="ewe-badge__label">{d.version}</span></span>
              {#if d.source === "aur"}<span class="ewe-badge ewe-badge--warning"><span class="ewe-badge__label">AUR</span></span>{/if}
            </div>
            <div class="ewe-row__desc">{d.description || d.source} · {formatDate(d.installedAt)}</div>
          </div>
          <div class="ewe-row__trail">
            <button
              class="ewe-btn ewe-btn--sm {confirming === `pkg:${d.package}` ? 'ewe-btn--danger' : 'ewe-btn--ghost'}"
              on:click={() => removeDebPkg(d)}
            >
              {confirming === `pkg:${d.package}` ? `Remove ${d.package}` : "Remove"}
            </button>
          </div>
        </div>
      {/each}
    {/if}
  </Group>
</Page>
