<script lang="ts">
	import '../app.css';
	import {
		DarkMode,
		Drawer,
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
	import { configuration, type Config } from '$lib/api';
	import { writable } from 'svelte/store';
	import { onMount } from 'svelte';

	let hide_sidebar = $state(false);
	let { children } = $props();

	function toggleSidebar() {
		hide_sidebar = !hide_sidebar;
	}
</script>

<svelte:window />
<div class="bg-slate-50 dark:bg-slate-900 p-4 w-full h-screen">
	<header>
		<Navbar class="rounded-md bg-slate-200 dark:bg-slate-800" let:NavContainer>
			<NavHamburger on:click={toggleSidebar} />
			<NavBrand href="/">
				<P size="xl">Chalkydri Manager</P>
			</NavBrand>
			<DarkMode />
		</Navbar>
	</header>

	<div class="flex flex-row">
		<Sidebar class="flexbox h-max">
			<SidebarWrapper class="bg-slate-200 dark:bg-slate-800 rounded-md mt-2 h-[100%]">
				<SidebarGroup>
					<SidebarItem label="Home" href="/" />
					<SidebarItem label="Calibration" href="/calibration" />
					<SidebarItem label="Settings" href="/settings" />
				</SidebarGroup>
			</SidebarWrapper>
		</Sidebar>

		<main class="flexbox w-full h-screen p-4">
			{@render children()}
		</main>
	</div>
</div>
