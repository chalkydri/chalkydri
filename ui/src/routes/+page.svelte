<script lang="ts">
	import { info, sysReboot, sysShutdown, restart, type Info } from '$lib/api';
	import {
		Button,
		Card,
		DarkMode,
		Layout,
		Modal,
		Navbar,
		NavBrand,
		NavHamburger,
		P,
		Sidebar,
		SidebarGroup,
		SidebarItem,
		SidebarWrapper
	} from 'flowbite-svelte';
	import { Clock3Icon, CpuIcon, FilmIcon, FrameIcon, HardDriveIcon, MemoryStickIcon } from 'lucide-svelte';
	import { onMount } from 'svelte';

	let req_reboot = $state(false);
	let req_shutdown = $state(false);

	let sys_info: Info | null = $state(null);

	onMount(() => {
		setInterval(async function() {
			let new_info = (await info()).data;
			if (new_info) {
				sys_info = new_info;
			}
		}, 500);
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

{#if sys_info}
<Layout cols="grid-cols-1 gap-2">
	<Card padding="sm" class="gap-1">
		<P size="lg">chalkydri</P>
		<div class="flex flex-col gap-1 ml-2">
			<div class="flex gap-2 items-center">
				<FilmIcon height="12" />
				<P>FPS</P>
			</div>
			<div class="flex gap-2 items-baseline">
				<CpuIcon height="12" />
				<P>{sys_info.cpu_usage}%</P>
			</div>
			<div class="flex gap-2">
				<MemoryStickIcon height="12" />
				<P>{sys_info.mem_usage}%</P>
			</div>
			<div class="flex gap-2 items-center">
				<HardDriveIcon />
				<P>3.5GB / 32GB</P>
			</div>
			<div class="flex gap-2 items-center">
				<Clock3Icon />
				<P>00:05:00</P>
			</div>
		</div>
	</Card>

	<Card size="md" padding="sm">
		<div class="flex flex-row gap-2">
			<Button
				on:click={_restart}
				color="green">Restart Chalkydri</Button>
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
