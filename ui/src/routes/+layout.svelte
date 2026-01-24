<script lang="ts">
	import { slide } from 'svelte/transition';
	import { connected as connected_, sys_info } from '$lib/index';
	import '../app.css';
	import {
		Button,
		DarkMode,
		Indicator,
		Label,
		Modal,
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
	import { CameraIcon, Hammer, HammerIcon, HomeIcon, PaintRollerIcon, PencilRulerIcon, SettingsIcon } from 'lucide-svelte';
	import type { Camera, Info } from '$lib/api';
	import CamConfig from './settings/+components/CamConfig.svelte';
	import { config, loadConfig, updateConfig } from '$lib/config';

	let hide_sidebar = $state(false);
  let new_cam_setup: Camera|null = $state(null);

	let connected = $state(false);
	connected_.subscribe((val: boolean) => {
		connected = val;
	});

  let info: Info | null = $state(null);
  let new_cams: Array<Camera> = $state(Array());
  sys_info.subscribe((val: Info | null) => {
    if (val) {
      val.new_cams.forEach((cam: Camera) => {
        new_cams.push(cam);
      });
    }
    info = val;
  });
	let { children } = $props();

	function toggleSidebar() {
		hide_sidebar = !hide_sidebar;
	}
</script>

<svelte:window />

{#each new_cams as cam}
<Toast transition={slide} class="rounded-md dark:bg-slate-700" position="top-right">
	<P>A new camera is available!</P>
  <P>{ cam.id }</P>
	<Button class="mt-2" color="blue" size="sm" on:click={async () => {
    await loadConfig();
    if (config && config.cameras) {
      config.cameras.push(cam);
    }
    await updateConfig(config!);
    new_cam_setup = cam;
  }}>Set up now</Button>
</Toast>
{/each}

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
					<SidebarItem label="Home" href="/#/">
						<svelte:fragment slot="icon">
							<HomeIcon />
						</svelte:fragment>
					</SidebarItem>
					<SidebarItem label="Custom code" href="/#/custom">
						<svelte:fragment slot="icon">
							<PaintRollerIcon />
						</svelte:fragment>
					</SidebarItem>
					<SidebarItem label="Settings" href="/#/settings">
						<svelte:fragment slot="icon">
							<SettingsIcon />
						</svelte:fragment>
					</SidebarItem>
					<SidebarGroup class="ml-3">
						<SidebarItem label="Front Right" href="/#/camera/Front Right">
							<svelte:fragment slot="icon">
								<CameraIcon />
							</svelte:fragment>
						</SidebarItem>
					</SidebarGroup>
					<SidebarItem label="Tools" href="/#/tools">
						<svelte:fragment slot="icon">
							<PencilRulerIcon />
						</svelte:fragment>
					</SidebarItem>
				</SidebarGroup>
			</SidebarWrapper>
		</Sidebar>

		<main class="flexbox w-full h-screen p-4">
			{@render children()}
		</main>
	</div>
</div>
