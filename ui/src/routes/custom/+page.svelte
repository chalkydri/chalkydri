<script lang="ts">
	import { Button, Card, Checkbox, P } from 'flowbite-svelte';
	import { PlusIcon, TrashIcon } from 'lucide-svelte';
	import Editor from './+components/Editor.svelte';
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
				{#each Object.keys($config.custom_subsystems) as subsystem}
					<Checkbox class="pr-2" />
					<P title="Edit the {subsystem} custom subsystem" size="lg" class="mr-auto hover:cursor-pointer" onclick={async () => {
						editing_subsys = subsystem;
					}}>{subsystem}</P>
					<Button title="Delete the {subsystem} custom subsystem" color="red" size="xs" class="ml-1"><TrashIcon size="14pt" /></Button>
				{/each}
			</div>
		</Card>
		</div>
		<Button title="Create a new custom subsystem" color="blue" size="md" class="w-min"><PlusIcon size="12pt" /></Button>
	</Card>
{/if}
