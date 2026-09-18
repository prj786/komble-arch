<script>
  import Row from "./Row.svelte";
  import Toggle from "./Toggle.svelte";
  export let title = "";
  export let sub = "";
  export let icon = "";
  export let on = false;
  export let dim = false;
  export let toggled = () => {};
</script>

<Row {title} {sub} {icon} {dim}>
  <Toggle {on} disabled={dim} label={title} {toggled} />
</Row>
