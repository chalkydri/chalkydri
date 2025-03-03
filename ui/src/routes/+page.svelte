<script lang="ts">
	import { sysInfo, sysReboot, sysShutdown } from '$lib/api';
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
	import { Clock3Icon, FilmIcon, FrameIcon, HardDriveIcon, MemoryStickIcon } from 'lucide-svelte';

	let req_reboot = false;
	let req_shutdown = false;

	async function _reboot() {
		await sysReboot();
	}
	async function _shutdown() {
		await sysShutdown();
	}
</script>

<Layout cols="grid-cols-1 gap-2">
	<Card padding="sm" class="gap-1">
		<P size="lg">chalkydri</P>
		<div class="flex flex-col gap-1 ml-2">
			<div class="flex gap-2 items-center">
				<FilmIcon height="12" />
				<P>FPS</P>
			</div>
			<!-- <div class="flex gap-2 items-baseline"></div> -->
			<div class="flex gap-2">
				<MemoryStickIcon height="12" />
				<P>2GB / 4GB</P>
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

	<Card padding="sm">
		<div class="flex flex-row gap-2">
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
