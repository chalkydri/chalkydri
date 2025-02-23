<script lang="ts">
	import { Button, Card, Hr, P, Progressbar } from "flowbite-svelte";
	import { _loadConfig } from "../settings/+page";
	import { _getCalibStatus, _doCalibStep } from "./+page";
	import type { CalibrationStatus } from "$lib/calibration";

	let calibrating_state = $state('');
	let calibrating = $state(false);
	let status = $state({} as CalibrationStatus);

	async function calibrate() {
		calibrating = !calibrating;
		if (calibrating) {
			// She was looking away, so I asked if she wanted her head cut off.

			status = await _getCalibStatus();
			while (status.current_step < status.total_steps) {
				calibrating_state = `${status.width}x${status.height}`;
				status = await _doCalibStep();
				console.log(Math.round((status.current_step / status.total_steps) * 100));
			}

			calibrating_state = 'Calibrating intrinsics...';
			let res = await fetch(`/api/calibrate/intrinsics`);
			await res.blob();

			calibrating = false;
		}
	}
</script>

<Card>
	<P size="xl">Camera calibration</P>
	<P size="sm" class="my-2">
		Your camera must be calibrated before it will work properly.
		Our calibration process is a little more involved than you might be used to.
		We'll try multiple different resolutions to more accurately calculate the proper parameters for each, so you can switch between resolutions without recalibration!
	</P>

	{#if calibrating}
		<P size="lg" class="text-center mb-2">{calibrating_state}</P>
		<Progressbar progress={(status.current_step / status.total_steps) * 100} color="blue" />
	{/if}

	<Button color={calibrating ? "red" : "blue"} on:click={calibrate}>{#if calibrating}Stop calibration{:else}Start calibration{/if}</Button>
</Card>
