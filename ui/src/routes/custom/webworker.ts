import { ServiceManager } from 'ace-linters/build/service-manager';
//import { AceLanguageClient } from "ace-linters/build/ace-language-client";
//import type { LanguageClientConfig } from "ace-linters/build/ace-language-client";

let manager = new ServiceManager(self);

//let mode = {name: "python", mode: "ace/mode/python", content: ''};
//const serverData: LanguageClientConfig = {
//    module: () => import("ace-linters/build/language-client"),
//    modes: "python",
//    type: "socket",
//    socket: new WebSocket("ws://localhost:3000"),
//		features: {
//			completion: true,
//			completionResolve: true,
//			hover: true,
//			documentHighlight: true,
//			codeAction: false,
//			diagnostics: false,
//		},
//}

manager.registerService('pythonls', {
	module: () => import('ace-linters/build/language-client'),
	modes: 'python',
	type: 'socket',
	socket: new WebSocket('ws://localhost:3000'),
	options: {
		pylsp: {
			configurationSources: ['pycodestyle'],
			plugins: {
				pycodestyle: {
					enabled: true,
					ignore: ['E501'],
					maxLineLength: 10
				},
				pyflakes: {
					enabled: false
				}
			}
		}
	},
	initializationOptions: {
		configuration: {
			svelte: {
				plugin: {
					typescript: { enable: false }
				}
			}
		}
	},
	features: {
		completion: true,
		completionResolve: true,
		hover: true,
		documentHighlight: true,
		codeAction: false,
		diagnostics: false
	}
});
