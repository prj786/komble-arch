<script>
	// Divider (design/system/components/Divider).
	import { Separator as SeparatorPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		orientation = "horizontal",
		"data-slot": dataSlot = "separator",
		...restProps
	} = $props();
</script>

<SeparatorPrimitive.Root
	bind:ref
	data-slot={dataSlot}
	{orientation}
	class={cn("ewe-divider", orientation === "vertical" && "ewe-divider--vertical ewe-divider--full", className)}
	{...restProps}
/>
