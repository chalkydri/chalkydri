<script lang="ts">
	import { slide } from 'svelte/transition';
	import { connected as connected_, sys_info } from '$lib/index';
	import '../app.css';
	import {
		Button,
		DarkMode,
		Indicator,
		Label,
		Navbar,
		NavBrand,
		NavHamburger,
		P,
		Sidebar,
		SidebarCta,
		SidebarDropdownItem,
		SidebarDropdownWrapper,
		SidebarGroup,
		SidebarItem,
		SidebarWrapper,
		Toast
	} from 'flowbite-svelte';
	import { CameraIcon, HomeIcon, PencilRulerIcon, SettingsIcon } from 'lucide-svelte';

	let hide_sidebar = $state(false);
	let connected = $state(false);
	connected_.subscribe((val: boolean) => {
		connected = val;
	});
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
				<img src="/icon.png" width="36pt" class="mr-2" alt="Chalkydri logo" />
				<P size="xl">Chalkydri</P>
			</NavBrand>
			<div class="ml-auto mr-4 flex flex-row items-center">
				{#if connected}
					<Indicator color="green" />
					<P class="pl-1">Connected</P>
				{:else}
					<Indicator color="red" />
					<P class="pl-1">Disconnected</P>
				{/if}
			</div>
			<DarkMode />
		</Navbar>
	</header>

	<div class="flex flex-row">
		<Sidebar class="flexbox h-max">
			<SidebarWrapper class="bg-slate-200 dark:bg-slate-800 rounded-md mt-2 h-[100%]">
				<SidebarGroup>
					<SidebarItem label="Home" href="/">
						<svelte:fragment slot="icon">
							<HomeIcon />
						</svelte:fragment>
					</SidebarItem>
					<SidebarItem label="Custom code" href="/custom">
						<svelte:fragment slot="icon">
							<PencilRulerIcon />
						</svelte:fragment>
					</SidebarItem>
					<SidebarItem label="Settings" href="/settings">
						<svelte:fragment slot="icon">
							<SettingsIcon />
						</svelte:fragment>
					</SidebarItem>
					<SidebarGroup class="ml-3">
						<SidebarItem label="Front Right" href="/camera">
							<svelte:fragment slot="icon">
								<CameraIcon />
							</svelte:fragment>
						</SidebarItem>
					</SidebarGroup>
				</SidebarGroup>
			</SidebarWrapper>
		</Sidebar>

		<main class="flexbox w-full h-screen p-4">
			{@render children()}
		</main>
	</div>
</div>
