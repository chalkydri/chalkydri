<script lang="ts">
	import { sys_info, connected as connected_ } from '$lib/index';
	import { sysReboot, sysShutdown, restart, type Info } from '$lib/api';
	import { Button, Card, Layout, Modal, P, Skeleton, Spinner } from 'flowbite-svelte';
	import { Clock3Icon, CpuIcon, FilmIcon, HardDriveIcon, MemoryStickIcon } from 'lucide-svelte';

	let req_reboot = $state(false);
	let req_shutdown = $state(false);

	let info: Info | null = $state(null);
	sys_info.subscribe((val: Info | null) => {
		info = val;
	});

	let connected = $state(false);
	connected_.subscribe((val: boolean) => {
		connected = val;
	});

	async function _restart() {
		await restart();
	}
	async function _reboot() {
		await sysReboot();
	}
	async function _shutdown() {
		await sysShutdown();
	}
</script>

{#if info}
	<Layout cols="grid-cols-1 gap-2">
		<Card padding="sm" class="gap-1">
			<P size="lg">chalkydri</P>
			<div class="flex flex-col gap-1 ml-2">
				<div class="flex gap-2 items-baseline">
					<FilmIcon height="12" />
					{#if connected}
						<P> FPS</P>
					{:else}
						<Spinner color="blue" size={4} class="my-auto" />
					{/if}
				</div>
				<div class="flex gap-2 items-baseline">
					<CpuIcon height="12" />
					{#if connected}
						<P>{info.cpu_usage}%</P>
					{:else}
						<Spinner color="blue" size={4} class="my-auto" />
					{/if}
				</div>
				<div class="flex gap-2 items-baseline">
					<MemoryStickIcon height="12" />
					{#if connected}
						<P>{info.mem_usage}%</P>
					{:else}
						<Spinner color="blue" size={4} class="my-auto" />
					{/if}
				</div>
				<div class="flex gap-2 items-baseline">
					<Clock3Icon height="12" />
					{#if connected}
						<P>{info.uptime}</P>
					{:else}
						<Spinner color="blue" size={4} class="my-auto" />
					{/if}
				</div>
			</div>
		</Card>

		<Card size="md" padding="sm">
			<div class="flex flex-row gap-2">
				<Button on:click={_restart} color="green">Restart Chalkydri</Button>
				<Button
					on:click={() => {
						req_reboot = true;
					}}
					color="blue">Reboot</Button
				>
				<Button
					on:click={() => {
						req_shutdown = true;
					}}
					color="red">Shutdown</Button
				>
			</div>
		</Card>
	</Layout>
{:else}
	<Skeleton />
{/if}

<Modal bind:open={req_reboot} autoclose outsideclose>
	<P>Are you sure you want to reboot?</P>
	<svelte:fragment slot="footer">
		<Button on:click={_reboot} color="red">Reboot</Button>
		<Button color="alternative">AAAAA</Button>
	</svelte:fragment>
</Modal>

<Modal bind:open={req_shutdown} autoclose outsideclose>
	<P>Are you sure you want to shut down?</P>
	<svelte:fragment slot="footer">
		<Button on:click={_shutdown} color="red">Shut down</Button>
		<Button color="alternative">AAAAA</Button>
	</svelte:fragment>
</Modal>
