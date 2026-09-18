<!-- A list well (List row card): surface-raised — surface-overlay inside the
     app pane — with a 1px border-subtle outline, rounded, 4px padding. Its
     rows are .ewe-row. -->
<script>
  let className = "";
  export { className as class };
</script>

<div class="ewe-list {className}">
  <slot />
</div>
