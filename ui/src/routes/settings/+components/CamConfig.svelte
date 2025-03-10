<script lang="ts">
	import { config } from '$lib/config';
	import { configuration, type Camera, type CameraSettings, type Config } from '$lib/api';
	import { Button, Card, Input, Label, Layout, P, Range, Select, Toggle } from 'flowbite-svelte';
	import { CheckIcon, PencilIcon, XIcon } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { derived, type Writable, writable } from 'svelte/store';

	let {
		disabled = $bindable(false),
		camera = $bindable(null)
	}: { disabled: boolean; camera: Camera | null } = $props();
	var camera_settings: { name: string; value: CameraSettings }[] = $state([]);
	let editing_name: string | null = $state(null);
	let field_layout_options: { name: string; value: string }[] | null = $state(null);

	function updateMappings() {
		if (config && config.field_layouts) {
			let keys = Object.keys(config.field_layouts);
			field_layout_options = keys.map((thing) => {
				return { name: thing, value: thing };
			});
		}
	}

	onMount(() => {
		updateMappings();
	});
</script>

<Card size="lg" padding="md" class="my-2">
	{#if camera}
		<div class="flex mb-1 items-center">
			{#if editing_name != null}
				<Input type="text" class="px-2 py-1" size="lg" bind:value={editing_name} />
				<Button
					size="xs"
					color="green"
					class="ml-1"
					on:click={() => {
						camera.name = editing_name;
						editing_name = null;
					}}
				>
					<CheckIcon size="14pt" />
				</Button>
				<Button
					size="xs"
					color="red"
					class="ml-1"
					on:click={() => {
						editing_name = null;
					}}
				>
					<XIcon size="14pt" />
				</Button>
			{:else}
				<P size="xl" class="mr-auto font-semibold">{camera.name}</P>
				<Button
					size="xs"
					color="blue"
					on:click={() => {
						editing_name = camera.name;
					}}
				>
					<PencilIcon size="14pt" />
				</Button>
			{/if}
		</div>

		<Layout gap="3">
			<div>
				<Label for="res_fps" class="mt-2 mb-1">Resolution / Frame Rate</Label>
				<Select
					id="res_fps"
					items={camera.possible_settings.map((val) => {
						return {
							name: `${val.width}x${val.height} @${val.frame_rate.num / val.frame_rate.den}fps`,
							value: val
						};
					})}
					bind:value={camera.settings}
				/>
			</div>

			{#if camera.settings}
				<Label class="mt-4 mb-2" for="gamma">Gamma</Label>
				<Range id="gamma" title="Gamma" min="-5.0" max="5.0" bind:value={camera.settings.gamma} />
			{/if}

			{#if camera.subsystems}
				<Card padding="sm" class="mt-2">
					<P size="lg">Subsystems</P>
					{#if camera.subsystems.capriltags}
						<Card padding="xs" class="mt-2">
							<Toggle color="blue" bind:disabled bind:checked={camera.subsystems.capriltags.enabled}
								>C AprilTags</Toggle
							>
							{#if camera.subsystems.capriltags.enabled && config.field_layouts}
								<Select
									bind:value={camera.subsystems.capriltags.field_layout}
									items={Object.keys(config.field_layouts).map((thing) => {
										return { name: thing, value: thing };
									})}
								/>
							{/if}
						</Card>
					{/if}
					<Card padding="xs" class="mt-2">
						<Toggle color="blue" bind:disabled bind:checked={camera.subsystems.ml.enabled}
							>Machine Learning</Toggle
						>
					</Card>
				</Card>
			{/if}
		</Layout>
	{/if}
</Card>
