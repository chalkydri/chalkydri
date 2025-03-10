<script lang="ts">
	import { Button, Card, Fileupload, Input, Label, Layout, P, Skeleton } from 'flowbite-svelte';
	import { _loadConfig, _saveConfig } from './+page';
	import { onMount } from 'svelte';
	import { configure, type Config, type Camera } from '$lib/api';
	import CamConfig from './+components/CamConfig.svelte';
	import { config, updateConfig, loadConfig } from '$lib/config';
	import { client } from '$lib/api/client.gen';

	let cfg: Config | null = $state(null);
	let saving = $state(false);

	async function save() {
		saving = true;

		let new_config = (
			await configure({
				body: cfg ? cfg : ({} as Config)
			})
		).data;
		if (new_config && cfg) {
			updateConfig(cfg);
		}

		saving = false;
	}

	let camera_mapping = $state({} as { name: string; value: Camera }[]);
	onMount(async () => {
		await loadConfig();
		cfg = config;
		if (config && config.cameras) {
			camera_mapping = config.cameras.map((val) => {
				return { name: val.name, value: val };
			});
		}
	});

	let files: FileList | undefined = $state();

	$effect(async () => {
		if (cfg && files) {
			for (const file of files) {
				if (!cfg.field_layouts) {
					cfg.field_layouts = {};
				}
				cfg.field_layouts[file.name] = JSON.parse(await file.text());
			}
			files = undefined;
		}
	});
</script>

{#if cfg}
	<Card padding="sm">
		<P size="lg">General</P>

		<Label for="device_name" class="mt-2 mb-1">Device name</Label>
		<Input id="device_name" bind:value={cfg.device_name} />

		<Fileupload bind:files />
	</Card>

	<!--
	<Card padding="sm">
		<P size="lg">Cameras</P>
	-->
	{#if cfg.cameras}
		{#each cfg.cameras as camera, i}
		<Layout>
			<CamConfig bind:camera={cfg.cameras[i]} bind:disabled={saving} />
			<Card padding="xs">
				<img src={`${client.getConfig().baseUrl}/stream/${camera.name}`} alt={camera.name} />
			</Card>
					</Layout>
		{/each}
	{/if}
	<!--
	</Card>
	-->

	<Card class="mt-2">
		<Button color="blue" on:click={save}
			>{#if saving}Saving...{:else}Save{/if}</Button
		>
	</Card>
{:else}
	<Skeleton />
{/if}
