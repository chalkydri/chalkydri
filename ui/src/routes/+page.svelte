<script lang="ts">
	import {
		Button,
		Card,
		DarkMode,
		Layout,
		Navbar,
		NavBrand,
		NavHamburger,
		P,
		Sidebar,
		SidebarGroup,
		SidebarItem,
		SidebarWrapper
	} from 'flowbite-svelte';
	import { HardDriveIcon, MemoryStickIcon } from 'lucide-svelte';

	async function _reboot() {
		let res = await fetch('/api/sys/reboot', {
			method: 'POST'
		});
		await res.blob();
	}
</script>

<h1>Hello, world!</h1>

<Button on:click={_reboot} color="red">Reboot</Button>
<Layout>
	<Card padding="sm">
		<Layout width="min-width">
			<MemoryStickIcon />
			<P>2GB / 4GB</P>
			<HardDriveIcon />
			<P>3.5GB / 32GB</P>
		</Layout>
	</Card>
</Layout>
