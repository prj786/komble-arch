<script>
  // Key–value, as a row in a list well: the key in text-muted, the value in
  // text-primary on the right (KeyValue card).
  export let k = "";
  export let v = "";
  export let mono = false;
</script>

<div class="ewe-row kv-row">
  <div class="ewe-row__text"><div class="ewe-row__title text-dim">{k}</div></div>
  <div class="ewe-row__trail kv-value {mono ? 'ewe-kv__mono' : ''}" title={v}>{v}</div>
</div>
