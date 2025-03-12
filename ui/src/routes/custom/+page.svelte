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

	import { Button, Card, P } from 'flowbite-svelte';
	import { onMount } from 'svelte';
	import type { Editor, EditSession } from 'ace-code';
	import { PencilIcon, PlusIcon, TrashIcon } from 'lucide-svelte';

	onMount(async () => {
		//let worker = new Worker(new URL('./webworker.ts', import.meta.url));
		//let languageProvider = LanguageProvider.create(worker);

		let editor = ace.edit('editor');
		editor.getSession().setMode('ace/mode/python');
		editor.setTheme('ace/theme/monokai');
		editor.setOptions({
			enableBasicAutocompletion: true,
			enableLiveAutocompletion: true,
			enableAutoIndent: true,
			enableSnippets: false,
			fontSize: 16,
			//enableCodeLens: true,
		});
		//languageProvider.registerEditor(editor as Editor);
		//languageProvider.setSessionOptions(editor.session as EditSession, {});
		//console.log(languageProvider.requireFilePath);
		//if (mode.filePath) {
		//	languageProvider.setSessionFilePath(editor.session, mode.filePath);
		//}
	});
</script>

<Card padding="sm" size="lg">
	<P size="xl">Custom subsystems</P>
	<P>You can create custom subsystems in Python!</P>

	<Card padding="xs" size="sm">
		<div class="flex flex-row items-center">
			<P size="lg" class="mr-auto">My subsys</P>
			<Button color="blue" size="xs"><PencilIcon size="14pt" /></Button>
			<Button color="red" size="xs" class="ml-1"><TrashIcon size="14pt" /></Button>
		</div>
	</Card>
	<Button color="blue" class="w-min"><PlusIcon /></Button>
</Card>

<Card padding="xs" size="xl">
	<div id="editor" style="height: 70vh"></div>
</Card>
