<script lang="ts">
	import { slide } from 'svelte/transition';
	import '../app.css';
	import {
	Button,
		DarkMode,
		Navbar,
		NavBrand,
		NavHamburger,
		P,
		Sidebar,
		SidebarGroup,
		SidebarItem,
		SidebarWrapper,

		Toast

	} from 'flowbite-svelte';

	let hide_sidebar = $state(false);
	let { children } = $props();

	function toggleSidebar() {
		hide_sidebar = !hide_sidebar;
	}
</script>

<svelte:window />

<Toast transition={slide} class="rounded-md dark:bg-slate-700" position="top-right">
	<P>A new camera is available!</P>
	<Button class="mt-2" color="blue" size="sm">Set up now</Button>
</Toast>

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
					<SidebarItem label="Custom subsystems" href="/custom" />
					<SidebarItem label="Settings" href="/settings" />
				</SidebarGroup>
			</SidebarWrapper>
		</Sidebar>

		<main class="flexbox w-full h-screen p-4">
			{@render children()}
		</main>
	</div>
</div>
