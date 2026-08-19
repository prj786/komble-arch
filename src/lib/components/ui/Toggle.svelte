<script>
  import { Switch } from "./switch/index.js";
  export let on = false;
  export let disabled = false;
  export let toggled = () => {};
</script>

<!-- Controlled: `on` stays the single source of truth, so the switch is told
     its state and only ever reports intent back through toggled(). The
     off-track override drops shadcn's dark 80% tint back to a flat zinc-600,
     the colour this toggle has always used. -->
<Switch
  checked={on}
  {disabled}
  aria-label="Toggle"
  class="dark:data-unchecked:bg-input"
  onCheckedChange={() => toggled()}
/>
