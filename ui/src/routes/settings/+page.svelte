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

	async function save() {
		saving = true;
		
		config = (await configure({
			body: config ? config : ({} as Config)
		})).data;
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
		return camera.subsystems.capriltags.field_layouts.keys;
	}

	let files: FileList | undefined = $state();
	let field_layout_options: { name: string, value: string }[] | undefined = $state();

	$effect(() => {
				if (config) {
					if (config.cameras) {
						config.cameras.forEach(async (cam) => {
							if (cam && files) {
								for (const file of files) {
									if (config && config.cameras) {
										config.cameras[config.cameras.indexOf(cam)].subsystems.capriltags.field_layouts[file.name] = JSON.parse(await file.text());
									}
								}
							}
						});

						let keys = Object.keys(config.cameras[0].subsystems.capriltags.field_layouts);
						field_layout_options = keys.map((thing) => { return { name: thing, value: thing }; });
					}
			}
		});

</script>

{#if config}
	<Card padding="sm">
		<P size="lg">General</P>

		<Label for="device_name" class="mt-2 mb-1">Device name</Label>
		<Input id="device_name" bind:value={config.device_name} />

		<Fileupload bind:files />
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
							{#if camera.subsystems.capriltags}
							<Card padding="xs" class="mt-2">
								<Toggle
									color="blue"
									disabled={saving}
									bind:checked={camera.subsystems.capriltags.enabled}>C AprilTags</Toggle
								>
		{#if camera.subsystems.capriltags.enabled && field_layout_options}
			<Select bind:value={camera.subsystems.capriltags.field_layout} items={field_layout_options} />
		{/if}
							</Card>
							{/if}
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
