<script>
	// Switch (design/system/components/Switch): bits-ui gives the role,
	// keyboard and checked state; the look is .ewe-switch. The on state comes
	// from bits-ui's data-state="checked" (components.css, Live states).
	import { Switch as SwitchPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";
	import Icon from "../Icon.svelte";

	let {
		ref = $bindable(null),
		class: className,
		checked = $bindable(false),
		size = "md",
		...restProps
	} = $props();
</script>

<SwitchPrimitive.Root
	bind:ref
	bind:checked
	data-slot="switch"
	class={cn("ewe-switch", size === "lg" && "ewe-switch--lg", className)}
	{...restProps}
>
	<!-- the thumb carries x / check, so the state doesn't rely on colour alone -->
	<SwitchPrimitive.Thumb data-slot="switch-thumb" class="ewe-switch__thumb">
		{#snippet children({ checked })}
			<Icon name={checked ? "check" : "x"} />
		{/snippet}
	</SwitchPrimitive.Thumb>
</SwitchPrimitive.Root>
