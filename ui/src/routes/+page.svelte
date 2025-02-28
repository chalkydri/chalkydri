<script lang="ts">
	import {
		Button,
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

	async function _reboot() {
		let res = await fetch('/api/sys/reboot', {
			method: 'POST'
		});
		await res.blob();
	}
</script>

<h1>Hello, world!</h1>

<Button on:click={_reboot} color="red">Reboot</Button>
