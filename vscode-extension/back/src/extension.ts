// The module 'vscode' contains the VS Code extensibility API
// Import the module and reference it with the alias vscode in your code below
import * as vscode from 'vscode';

import {register_view} from './view';

export function activate(context: vscode.ExtensionContext) {

	vscode.workspace.findFiles("config.toml")
		.then(files => {
			let file = null;
			if (files.length === 1) {
				file = files[0];
			}
			register_view(file, context);
		});
}

// This method is called when your extension is deactivated
export function deactivate() {}
