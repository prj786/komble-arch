<script>
  import { Switch } from "./switch/index.js";
  export let on = false;
  export let disabled = false;
  export let toggled = () => {};
</script>

<!-- Controlled: `on` stays the single source of truth, so the switch is told
     its state and only ever reports intent back through toggled(). The
     switch itself paints the ewe roles (brand-bg on, --bg-2 off). -->
<Switch
  checked={on}
  {disabled}
  aria-label="Toggle"
  onCheckedChange={() => toggled()}
/>
