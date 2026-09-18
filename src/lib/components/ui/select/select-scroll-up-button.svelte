<script>
	import { Select as SelectPrimitive } from "bits-ui";
	import Icon from "../Icon.svelte";
	import { ICONS } from "../icons.js";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		class: className,
		...restProps
	} = $props();
</script>

<SelectPrimitive.ScrollUpButton
	bind:ref
	data-slot="select-scroll-up-button"
	class={cn("flex items-center justify-center text-dim", className)}
	{...restProps}
>
	<Icon code={ICONS.caretUp} size="sm" />
</SelectPrimitive.ScrollUpButton>