// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import {register_view} from './view';
import {reSsg_server_watcher, ReSsgController} from './server-watcher';

let config_toml: vscode.Uri | undefined = undefined;
let ressg_controller: ReSsgController | undefined = undefined;

export function activate(context: vscode.ExtensionContext) {

	vscode.workspace.findFiles("config.toml")
		.then(files => {
			let file = null;
			if (files.length === 1) {
				file = files[0];
			}
			config_toml = file;
		});

	ressg_controller = reSsg_server_watcher(config_toml);
	register_view(ressg_controller, context);
}

// This method is called when your extension is deactivated
export function deactivate() {
	ressg_controller.abort();
}
