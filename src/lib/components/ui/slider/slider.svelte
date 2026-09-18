<script>
	// Slider (design/system/components/Slider): bits-ui for pointer, keyboard
	// and ARIA; .ewe-slider__track / __fill / __thumb for the look. The root
	// is a control-sm tall hit area around the 4px track, so the track is easy
	// to grab. bits-ui positions the fill and thumb inline (left / right /
	// translate), which is why the thumb only keeps its vertical centring here.
	import { Slider as SliderPrimitive } from "bits-ui";
	import { cn } from "$lib/utils.js";

	let {
		ref = $bindable(null),
		value = $bindable(),
		class: className,
		...restProps
	} = $props();
</script>

<SliderPrimitive.Root
	bind:ref
	bind:value
	data-slot="slider"
	class={cn("slider-hit", className)}
	{...restProps}
>
	{#snippet children({ thumbItems })}
		<span class="ewe-slider__track" data-slot="slider-track">
			<SliderPrimitive.Range data-slot="slider-range" class="ewe-slider__fill" />
		</span>
		{#each thumbItems as thumb (thumb.index)}
			<SliderPrimitive.Thumb data-slot="slider-thumb" index={thumb.index} class="ewe-slider__thumb" />
		{/each}
	{/snippet}
</SliderPrimitive.Root>
