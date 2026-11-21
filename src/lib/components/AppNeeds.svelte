<script>
  // What an installed app still needs before it works (needs.rs), with the
  // one-click fix: Stremio Enhanced's streaming server is the first. Shown on
  // the app's page and under its row in Installed. Renders nothing for the
  // apps (nearly all) that need nothing.
  import { toast } from "../stores";
  import * as api from "../api";
  import Icon from "./ui/Icon.svelte";

  /** AppImage id or package name; null = not installed, show nothing */
  export let id = null;
  /** the app's name, for the toast */
  export let name = "";

  let needs = [];
  let fixing = null;

  $: load(id);
  async function load(key) {
    if (!key) {
      needs = [];
      return;
    }
    try {
      const r = await api.appNeeds(key);
      if (key === id) needs = (r.needs || []).filter((n) => !n.ok);
    } catch {
      needs = [];
    }
  }

  async function fix(n) {
    if (fixing) return;
    fixing = n.id;
    try {
      await api.fixAppNeed(n.id);
      toast(`Done. Restart **${name}** to use it.`, "success");
      await load(id);
    } catch (e) {
      toast(e, "error");
    }
    fixing = null;
  }
</script>

{#each needs as n (n.id)}
  <div class="ewe-alert ewe-alert--warning need" role="alert">
    <Icon name="warning" />
    <div class="ewe-alert__body">
      <div class="ewe-alert__title">{n.title}</div>
      <div class="ewe-alert__desc">{n.detail}</div>
      <div class="ewe-alert__actions">
        <button class="ewe-btn ewe-btn--sm ewe-btn--primary" disabled={fixing !== null} on:click={() => fix(n)}>
          <Icon name="download" />{fixing === n.id ? "Getting it…" : n.action}
        </button>
      </div>
    </div>
  </div>
{/each}

<style>
  .need {
    margin: var(--space-s) 0;
  }
</style>
