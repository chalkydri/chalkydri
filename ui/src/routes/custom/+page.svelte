<script lang="ts">
	import { Button, Card, Checkbox, Input, P } from 'flowbite-svelte';
	import { ArrowLeftIcon, CheckIcon, PencilIcon, PlusIcon, TrashIcon, XIcon } from 'lucide-svelte';
	import Editor from './+components/Editor.svelte';
	import type { CustomSubsystem } from '$lib/api';
	import { config } from '$lib';

	let editing_subsys: string | null = $state(null);
</script>

{#if editing_subsys}
	<Editor bind:name={editing_subsys} />
{:else}
	<Card padding="sm" size="lg">
		<P size="xl">Custom subsystems</P>
		<P size="sm">You can create custom subsystems in Python!</P>
	
		<div class="py-2">
	
		<Card padding="xs" size="sm">
			<div class="flex flex-row items-center">
				<!--{#if editing_subsys_name != null}
					<Input type="text" class="px-2 py-1" size="lg" bind:value={editing_subsys_name} />
					<Button
						size="xs"
						color="green"
						class="ml-1"
						on:click={() => {
							if (editing_subsys_name) {
								// = editing_subsys_name;
							}
							editing_subsys_name = null;
						}}
					>
						<CheckIcon size="14pt" />
					</Button>
					<Button
						size="xs"
						color="red"
						class="ml-1"
						on:click={() => {
							editing_subsys_name = null;
						}}
					>
						<XIcon size="14pt" />
					</Button>
				{:else}
				-->
				{#each Object.keys($config.custom_subsystems) as subsystem}
					<Checkbox class="pr-2" />
					<P size="lg" class="mr-auto hover:cursor-pointer" onclick={async () => {
						editing_subsys = subsystem;
					}}>{subsystem}</P>
					<Button color="red" size="xs" class="ml-1"><TrashIcon size="14pt" /></Button>
				<!--{/if}-->
				{/each}
			</div>
		</Card>
		</div>
		<Button color="blue" size="md" class="w-min"><PlusIcon size="12pt" /></Button>
	</Card>
{/if}
