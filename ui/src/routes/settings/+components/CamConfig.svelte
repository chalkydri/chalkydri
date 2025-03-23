<script lang="ts">
	import { config } from '$lib/config';
	import {
		configuration,
		type Camera,
		type CameraSettings,
		type Config,
		type VideoOrientation
	} from '$lib/api';
	import { Button, Card, Input, Label, Layout, Modal, P, Range, Select, Toggle } from 'flowbite-svelte';
	import { CheckIcon, PencilIcon, XIcon } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { derived, type Writable, writable } from 'svelte/store';
	import Calibration from '../../+components/Calibration.svelte';

	let {
		disabled = $bindable(false),
		camera = $bindable(null)
	}: { disabled: boolean; camera: Camera | null } = $props();
	let calibrating = $state(false);
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
						if (editing_name) {
							camera.name = editing_name;
						}
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

		<Layout gap={3}>
			{#if camera.possible_settings}
				<div>
					<Label for="res_fps" class="mt-2 mb-1">Resolution / Frame Rate</Label>
					<Select
						id="res_fps"
						items={camera.possible_settings.map((val) => {
							if (!val.frame_rate) {
								return {
									name: `${val.width}x${val.height} @?fps (${val.format})`,
									value: JSON.stringify(val)
								};
							} else {
								return {
									name: `${val.width}x${val.height} @${val.frame_rate.num / val.frame_rate.den}fps (${val.format})`,
									value: JSON.stringify(val)
								};
							}
						})}
						value={JSON.stringify(camera.settings)}
						on:input={(e) => {
							camera.settings = JSON.parse(e.target.value);
						}}
					/>
				</div>
			{/if}

			<div>
				<Label class="mt-4 mb-2" for="auto_exposure">Auto-exposure</Label>
				<Toggle bind:checked={camera.auto_exposure} />

				{#if !camera.auto_exposure}
					<Label class="mt-4 mb-2" for="manual_exposure">Exposure time</Label>
					<div class="flex flex-row gap-4 items-center">
						<Range
							id="manual_exposure"
							title="Exposure time"
							min="1"
							max="1000"
							step="1"
							bind:value={camera.manual_exposure}
						/>
						<Input type="number" bind:value={camera.manual_exposure} />
					</div>
				{/if}
			</div>

			<div>
				<Label for="orientation" class="mt-2 mb-1">Orientation</Label>
				<Select
					id="orientation"
					items={[
						{ name: '0', value: 'none' },
						{ name: '90', value: 'clockwise' },
						{ name: '180', value: 'rotate-180' },
						{ name: '270', value: 'counterclockwise' }
					]}
					bind:value={camera.orientation}
				/>
			</div>

			<div>
				<Button color="blue" on:click={() => { calibrating = true; }}>Calibrate</Button>
			</div>

			{#if camera.subsystems}
				<Card padding="lg" class="mt-2 col-span-2">
					<P size="lg">Subsystems</P>
					{#if camera.subsystems.capriltags}
						<Card padding="xs" class="mt-2">
							<Toggle color="blue" bind:disabled bind:checked={camera.subsystems.capriltags.enabled}
								>C AprilTags</Toggle
							>
							{#if config}
								{#if camera.subsystems.capriltags.enabled && config.field_layouts}
									<Select
										bind:value={camera.subsystems.capriltags.field_layout}
										items={Object.keys(config.field_layouts).map((thing) => {
											return { name: thing, value: thing };
										})}
									/>
								{/if}
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

<Modal bind:open={calibrating}>
	{#if camera}
		<Calibration bind:cam_name={camera.name} />
	{/if}
</Modal>
