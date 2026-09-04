<script>
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { route, updatesCount } from "../stores";

  // Real app version, not a hardcoded string that goes stale on release.
  let version = "";
  onMount(async () => {
    try {
      version = await getVersion();
    } catch {
      // dev server outside Tauri
    }
  });

  // Lucide codepoints — same icon language as the ewe DE
  const items = [
    { id: "discover", label: "Discover", icon: 0xE09B },   // compass
    { id: "foryou", label: "For you", icon: 0xE19F },      // user
    { id: "installed", label: "Installed", icon: 0xE129 }, // package
    { id: "updates", label: "Updates", icon: 0xE145 },     // arrows-clockwise
    { id: "aur", label: "AUR", icon: 0xE0B2 },             // download-simple
    { id: "settings", label: "Settings", icon: 0xE29A }    // faders
  ];
</script>

<!-- Collapses to an icon rail on narrow windows (tiled half/quarter screens). -->
<!-- .rail / .rail-* — the chrome shared with ewe-settings and ewe-sync (app.css) -->
<aside class="rail">
  <div class="rail-brand">
    <!-- the kombali — a shepherd's crook; Komble herds your apps -->
    <div
      class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg text-white"
      style="background: linear-gradient(135deg, #f0a65c, #744664)"
    >
      <svg viewBox="0 0 24 24" class="h-5 w-5" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M14.5 21V8a3.5 3.5 0 0 0-7 0v1.5" />
      </svg>
    </div>
    <div class="rail-brand-name">Komble</div>
  </div>

  <nav class="rail-nav">
    {#each items as it}
      <button
        title={it.label}
        class="rail-item {$route === it.id ? 'is-active' : ''}"
        on:click={() => route.set(it.id)}
      >
        <span class="icon">{String.fromCodePoint(it.icon)}</span>
        <span class="rail-label">{it.label}</span>
        {#if it.id === "updates" && $updatesCount > 0}
          <span
            class="hidden rounded-full px-1.5 py-0.5 text-[11px] font-semibold leading-none md:inline
              {$route === it.id ? 'bg-white/25 text-white' : 'text-white'}"
            style={$route === it.id ? "" : "background: var(--accent)"}
          >
            {$updatesCount}
          </span>
          <!-- icon-rail equivalent of the count badge -->
          <span
            class="absolute right-1.5 top-1.5 h-1.5 w-1.5 rounded-full md:hidden
              {$route === it.id ? 'bg-white' : ''}"
            style={$route === it.id ? "" : "background: var(--accent)"}
          ></span>
        {/if}
      </button>
    {/each}
  </nav>

  <div class="rail-foot">
    Komble{version ? ` ${version}` : ""} · no snap, no flatpak
  </div>
</aside>
