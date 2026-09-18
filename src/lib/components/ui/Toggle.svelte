<script>
  import { Switch } from "./switch/index.js";
  export let on = false;
  export let disabled = false;
  export let label = "";
  export let toggled = () => {};
</script>

<!-- Controlled: `on` stays the single source of truth, so the switch is told
     its state and only ever reports intent back through toggled(). -->
<Switch
  checked={on}
  {disabled}
  aria-label={label || "Toggle"}
  onCheckedChange={(v) => toggled(v)}
/>
