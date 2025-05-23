<script lang="ts">
	import {
		Button,
		Card,
		Fileupload,
		Input,
		Label,
		Layout,
		Modal,
		P,
		Select,
		Skeleton
	} from 'flowbite-svelte';
	import { _loadConfig, _saveConfig } from './+page';
	import { onMount } from 'svelte';
	import { configure, saveConfiguration, type Config, type Camera, restart } from '$lib/api';
	import CamConfig from './+components/CamConfig.svelte';
	import { config, updateConfig, loadConfig } from '$lib/config';
	import CameraFeed from '../+components/CameraFeed.svelte';
	import { TrashIcon } from 'lucide-svelte';
	import Calibration from '../+components/Calibration.svelte';

	let cfg: Config | null = $state(null);
	let saving = $state(false);
	let managing_field_layouts = $state(false);

	let calibrating = $state(false);
	let calibrating_cam_name: string | null = $state(null);

	async function save() {
		saving = true;

		let new_config = (
			await saveConfiguration({
				body: cfg ? cfg : ({} as Config)
			})
		).data;
		if (new_config && cfg) {
			updateConfig(cfg);
		}

		saving = false;

		await restart();
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
		let new_config = (
			await configure({
				body: cfg ? cfg : ({} as Config)
			})
		).data;
		if (new_config && cfg) {
			updateConfig(cfg);
		}
	});
</script>

{#if cfg}
	<Card padding="sm">
		<P size="lg">General</P>

		<Label for="team_num" class="mt-2 mb-1">Team number</Label>
		<Input id="team_num" type="number" bind:value={cfg.team_number} />

		<Label for="device_name" class="mt-2 mb-1">Device name</Label>
		<Input id="device_name" bind:value={cfg.device_name} />

		<Fileupload bind:files />
	</Card>

	<Card size="md" padding="sm" class="mt-2">
		<P size="lg">Pose estimation</P>

		<Layout gap={3}>
			<div>
				<Label for="field_layout" class="mt-2 mb-1">Field layout</Label>
				{#if config && config.field_layouts}
					<Select
						id="field_layout"
						items={Object.keys(config.field_layouts).map((thing) => {
							return { name: thing, value: thing };
						})}
					/>
				{/if}
			</div>

			<Button
				size="sm"
				class="m-auto"
				color="blue"
				on:click={() => {
					managing_field_layouts = true;
				}}>Manage field layouts</Button
			>
		</Layout>
	</Card>

	{#if cfg.cameras}
		{#each cfg.cameras as camera, i}
			<Layout>
				<CamConfig bind:camera={cfg.cameras[i]} bind:disabled={saving} />
				<div class="m-2">
					<CameraFeed bind:camera={cfg.cameras[i]} />
				</div>
			</Layout>
		{/each}
	{/if}

	<Card class="mt-2">
		<Button color="blue" on:click={save}
			>{#if saving}Saving...{:else}Save{/if}</Button
		>
	</Card>

	<Modal bind:open={managing_field_layouts} autoclose outsideclose>
		<P size="lg">Field layouts</P>

		{#if cfg.field_layouts}
			{#each Object.keys(cfg.field_layouts) as name}
				<Card padding="xs">
					<Layout>
						<div>
							<P>{name}</P>
							<P color="gray"
								>{cfg.field_layouts[name].field.length}m x {cfg.field_layouts[name].field.width}m</P
							>
							<Button size="xs" color="red">
								<TrashIcon size="14pt" />
							</Button>
						</div>
					</Layout>
				</Card>
			{/each}
		{/if}
	</Modal>

	<Modal bind:open={calibrating} autoclose outsideclose>
		<Calibration bind:cam_name={calibrating_cam_name} />
	</Modal>
{:else}
	<Skeleton />
{/if}
