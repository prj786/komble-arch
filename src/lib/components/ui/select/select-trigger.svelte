<script>
	// Select trigger (design/system/components/Select): the Text field box
	// with the current value and a chevron-down in text-secondary at the end.
	import { Select as SelectPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		children,
		size = "md",
		...restProps
	} = $props();
</script>

<SelectPrimitive.Trigger
	bind:ref
	data-slot="select-trigger"
	class={cn("ewe-input ewe-select", size === "sm" && "ewe-input--sm", size === "lg" && "ewe-input--lg", className)}
	{...restProps}
>
	<span class="ewe-input__text">{@render children?.()}</span>
	<Icon name="caretDown" size={size === "sm" ? "sm" : "md"} />
</SelectPrimitive.Trigger>
