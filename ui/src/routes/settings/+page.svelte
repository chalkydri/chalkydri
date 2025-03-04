<script lang="ts">
	import {
		Button,
		Card,
		Fileupload,
		Input,
		Label,
		MultiSelect,
		P,
		Select,
		Skeleton,
		Toggle
	} from 'flowbite-svelte';
	import { _loadConfig, _saveConfig } from './+page';
	import { onMount } from 'svelte';
	import {
		configure,
		configuration,
		type Config,
		type Camera,
		type CameraSettings
	} from '$lib/api';

	let saving = $state(false);

	function save() {
		saving = true;
		configure({
			body: config ? config : ({} as Config)
		});
		saving = false;
	}

	let config = $state(null as Config | undefined | null);
	let camera_mapping = $state({} as { name: string; value: Camera }[]);
	onMount(async () => {
		config = (await configuration()).data;
		if (config && config.cameras) {
			camera_mapping = config.cameras.map((val) => {
				return { name: val.display_name, value: val };
			});
		}
	});

	let camera: Camera | null = $state(null);

	function getSettings(camera: Camera) {
		if (camera.possible_settings) {
			return camera.possible_settings.map((s) => {
				return {
					name: `${s.width}x${s.height} @${s.frame_rate.num / s.frame_rate.den}fps`,
					value: s
				};
			});
		}
	}
	function getFieldLayouts(camera: Camera) {
		if (camera.possible_settings) {
			for ([key] in camera.subsystems.capriltags.field_layouts) {
				return {
					name: `${s.width}x${s.height} @${s.frame_rate.num / s.frame_rate.den}fps`,
					value: s
				};
			}
		}
	}

	let field_layout_files: FileList | undefined = $state();
	async function add_field_layouts() {
		console.log("bs");
	}
</script>

{#if config}
	<Card padding="sm">
		<P size="lg">General</P>

		<Label for="device_name" class="mt-2 mb-1">Device name</Label>
		<Input id="device_name" bind:value={config.device_name} />
	</Card>

	<Card padding="sm">
		<P size="lg">Cameras</P>

		{#if config.cameras}
			{#each config.cameras as camera}
				<Card padding="xs" class="my-2">
					<P>{camera.display_name}</P>

					<Label for="res_fps" class="mt-2 mb-1">Resolution / Frame Rate</Label>
					<Select id="res_fps" items={getSettings(camera)} bind:value={camera.settings} />

					<!--
					<Label for="subsystems" class="mt-2 mb-1">Subsystems</Label>
					<MultiSelect
						id="subsystems"
						items={[
							{ name: 'CAprilTags', value: !camera.subsystems.capriltags.enabled },
							{ name: 'Machine Learning', value: !camera.subsystems.ml.enabled }
						]}
					/>
					-->

					<!--
				<Label class="mt-4 mb-2" for="gamma">Gamma</Label>
				<Range id="gamma" title="Gamma" color="blue" />
				-->
					{#if camera.subsystems}
						<Card padding="sm" class="mt-2">
							<P size="lg">Subsystems</P>
							<Card padding="xs" class="mt-2">
								<Toggle
									color="blue"
									disabled={saving}
									bind:checked={camera.subsystems.capriltags.enabled}>C AprilTags</Toggle
								>
		{#if camera.subsystems.capriltags.enabled}
			<Select bind:value={camera.subsystems.capriltags.field_layout} />
			<Fileupload on:input={add_field_layouts} bind:files={field_layout_files} />
		{/if}
							</Card>
							<!--
	<Card padding="xs">
		<Toggle color="blue" disabled={saving} bind:checked={config.subsystems.apriltags.enabled}>AprilTags</Toggle>
	</Card>
	-->
							<Card padding="xs" class="mt-2">
								<Toggle color="blue" disabled={saving} bind:checked={camera.subsystems.ml.enabled}
									>Machine Learning</Toggle
								>
							</Card>
						</Card>
					{/if}
				</Card>
			{/each}
		{/if}
	</Card>

	<Card class="mt-2">
		<Button color="blue" on:click={save}
			>{#if saving}Saving...{:else}Save{/if}</Button
		>
	</Card>
{:else}
	<Skeleton />
{/if}
