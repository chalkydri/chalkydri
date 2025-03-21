<script lang="ts">
	import { Button, Card, Hr, Layout, P, Progressbar, Select } from 'flowbite-svelte';
	import { _loadConfig } from '../settings/+page';
	import { _getCalibStatus, _doCalibStep } from './+page';
	import type { CalibrationStatus } from '$lib/calibration';
	import { onMount } from 'svelte';
	import {
		calibrationIntrinsics,
		calibrationStatus,
		calibrationStep,
		configuration,
		type Camera,
		type Config
	} from '$lib/api';
	import CameraFeed from '../+components/CameraFeed.svelte';

	let config = $state(null as Config | undefined | null);
	let camera_mapping: { name: string; value: string }[] = $state([]);
	onMount(async () => {
		config = (await configuration()).data;
		if (config && config.cameras) {
			camera_mapping = config.cameras.map((val) => {
				return { name: val.name, value: val.id };
			});
		}
	});

	let cam_name: string | null = $state(null);
	let calibrating_state = $state('');
	let calibrating = $state(false);
	let status = $state({} as CalibrationStatus | null | undefined);

	async function calibrate(cam_name: string) {
		calibrating = !calibrating;
		if (calibrating) {
			// She was looking away, so I asked if she wanted her head cut off.

			status = (await calibrationStatus()).data;
			if (status) {
				while (status && status.current_step < status.total_steps) {
					calibrating_state = `${status.width}x${status.height}`;
					status = (await calibrationStep({ path: { cam_name: cam_name } }))
						.data as CalibrationStatus;
					if (status) {
						console.log(Math.round((status.current_step / status.total_steps) * 100));
					}
				}
			}

			calibrating_state = 'Calibrating intrinsics...';

			await calibrationIntrinsics({
				path: {
					cam_name: cam_name
				}
			});

			calibrating = false;
		}
	}
</script>

<Layout>
	<Card>
		<P size="xl">Camera calibration</P>
		<P size="sm" class="my-2">
			Your camera must be calibrated before it will work properly. Our calibration process is a
			little more involved than you might be used to. We'll try multiple different resolutions to
			more accurately calculate the proper parameters for each, so you can switch between
			resolutions without recalibration!
		</P>

		<Select class="mb-2" items={camera_mapping} bind:value={cam_name} />

		{#if calibrating}
			<P size="lg" class="text-center mb-2">{calibrating_state}</P>
			{#if status}
				<Progressbar progress={(status.current_step / status.total_steps) * 100} color="blue" />
			{/if}
		{/if}

		<Button
			class="mt-2"
			color={calibrating ? 'red' : 'blue'}
			disabled={!cam_name}
			on:click={async () => {
				await calibrate(cam_name as string);
			}}
			>{#if calibrating}Stop calibration{:else}Start calibration{/if}</Button
		>
	</Card>
	{#if cam_name && config && config.cameras}
		<CameraFeed
			camera={config.cameras.filter((cam) => {
				return cam_name == cam.id;
			})[0]}
		/>
	{/if}
</Layout>
