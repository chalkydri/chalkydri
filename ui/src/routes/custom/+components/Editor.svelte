<script lang="ts">
	import * as ace from 'ace-code';

	import 'ace-code/esm-resolver';
	import 'ace-code/src/ext/language_tools';
	//import 'ace-builds/src-noconflict/mode-python';
	//import 'ace-builds/src-noconflict/theme-monokai';
	//import 'ace-builds/src-noconflict/ext-language_tools';
	//import 'ace-builds/src-noconflict/ext-code_lens';
	//import 'ace-builds/src-noconflict/ext-inline_autocomplete';
	//import 'ace-builds/src-noconflict/ext-prompt';

	import { LanguageProvider } from 'ace-linters';

	import { Button, Card, Checkbox, Input, P } from 'flowbite-svelte';
	import { onDestroy, onMount } from 'svelte';
	import type { Editor, EditSession } from 'ace-code';
	import { ArrowLeftIcon, CheckIcon, PencilIcon, PlusIcon, SaveIcon, TrashIcon, XIcon } from 'lucide-svelte';
	import { configure, saveConfiguration, type CustomSubsystem } from '$lib/api';
	import { config } from '$lib';
	import { updateConfig } from '$lib/config';

	let code: string | null = $state(null);
	let editing_subsys_name: string | null = $state(null);

	let {
		name = $bindable(),
	}: {
		name: string | null,
	} = $props();

	let editor: Editor | null = null;
	let subsys: CustomSubsystem | null = $state(null);

	onMount(() => {
		while (!subsys) {
			if (name) {
				subsys = $config.custom_subsystems[name];
			}
		}

		if (subsys) {
			code = subsys.code;
		}

		//let worker = new Worker(new URL('./webworker.ts', import.meta.url));
		//let languageProvider = LanguageProvider.create(worker);

		editor = ace.edit('editor');
		editor.getSession().setMode('ace/mode/python');
		editor.setTheme('ace/theme/monokai');
		editor.setOptions({
			enableBasicAutocompletion: true,
			enableLiveAutocompletion: true,
			enableAutoIndent: true,
			enableSnippets: false,
			fontSize: 16
			//enableCodeLens: true,
		});
		if (subsys) {
			editor.setValue(subsys.code);
			editor.clearSelection();
		}
		//languageProvider.registerEditor(editor as Editor);
		//languageProvider.setSessionOptions(editor.session as EditSession, {});
		//console.log(languageProvider.requireFilePath);
		//if (mode.filePath) {
		//	languageProvider.setSessionFilePath(editor.session, mode.filePath);
		//}
	});
</script>

<Card padding="xs" size="xl" class="mb-2">
	<div class="flex flex-row items-center">
		<Button size="sm" onclick={() => {
			name = '';
		}}>
			<ArrowLeftIcon />
		</Button>
				{#if editing_subsys_name != null}
					<Input type="text" class="px-2 py-1" size="lg" bind:value={editing_subsys_name} />
					<Button
						size="xs"
						color="green"
						class="ml-1"
						on:click={async () => {
					if (editing_subsys_name && name && subsys) {
							let cfg = $config;
							cfg.custom_subsystems[editing_subsys_name] = subsys;
							delete cfg.custom_subsystems[name];
							let new_config = (await configure({
								body: cfg
							})).data;
							if (new_config) {
								updateConfig(new_config);
							}
						}
						name = editing_subsys_name;
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
					<P size="lg" class="w-[30%] hover:cursor-text" onclick={async () => {
						if (subsys) {
							editing_subsys_name = name;
						}
					}}>{name}</P>
					<Button color="red" size="xs" class="ml-1"><XIcon size="14pt" /></Button>
					<Button color="blue" size="xs" class="ml-1" onclick={async () => {
						if (name && subsys) {
							let cfg = $config;
							if (editor) {
								subsys.code = editor.getValue();
							}
							cfg.custom_subsystems[name] = subsys;
							let new_config = (await saveConfiguration({
								body: cfg
							})).data;
							if (new_config) {
								updateConfig(new_config);
							}
						}
						name = null;
					}}><SaveIcon size="14pt" /></Button>
				{/if}
			</div>
</Card>
<Card padding="xs" size="xl">
	<div id="editor" style="height: 70vh"></div>
</Card>
