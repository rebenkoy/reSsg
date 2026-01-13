import * as vscode from 'vscode';
import {ReSsgController as ReSsgController} from './server-watcher';
import { GitWorks } from './git-works';

export function register_view(ressg_controller: ReSsgController, context: vscode.ExtensionContext) {
    const reSsg_view_provider = new ReSsgViewProvider(ressg_controller, context.extensionUri);
    context.subscriptions.push(vscode.window.registerWebviewViewProvider(ReSsgViewProvider.viewType, reSsg_view_provider));
}

interface Message{
	type: string
}

class setServerProcessStatusMessage implements Message {
	type = 'setServerProcessStatus';
	is_active: boolean;
	constructor(is_active: boolean) {
		this.is_active = is_active;
	}
}
class setBranchMessage implements Message {
	type = 'setBranch';
	branch_name: string;
	constructor(branch_name: string) {
		this.branch_name = branch_name;
	}
}

class tellSaveStatus implements Message {
	type = 'tellSaveStatus';
	ok: boolean;
	constructor(ok: boolean) {
		this.ok = ok;
	}
}


class ReSsgViewProvider implements vscode.WebviewViewProvider {

	public static readonly viewType = 'reSsg.general';

	private _view?: vscode.WebviewView;
    private controller: ReSsgController | null;
    private updater?: ReturnType<typeof setInterval>;
    private gitController: GitWorks;

	constructor(
        ressg_controller: ReSsgController,
		private readonly _extensionUri: vscode.Uri,
	) { 
        this.controller = ressg_controller;
        this.gitController = new GitWorks();
    }

	public resolveWebviewView(
		webviewView: vscode.WebviewView,
		_context: vscode.WebviewViewResolveContext,
		_token: vscode.CancellationToken,
	) {
		this._view = webviewView;

		webviewView.webview.options = {
			// Allow scripts in the webview
			enableScripts: true,

			localResourceRoots: [
				this._extensionUri
			]
		};
        
		webviewView.webview.html = this._getHtmlForWebview(webviewView.webview);

		webviewView.webview.onDidReceiveMessage(data => {
			switch (data.type) {
				case 'save':
					{
						console.log("Saving:", data);
						this.gitController.save(data.branch_name, data.commit_message)
							.then(res => {
								this.postMessage(new tellSaveStatus(res));
							});
						break;
					}
			}
		});
        
        this.updater = setInterval(() => {
            this.updateBranch();
            this.setServerProcessStatus();
        }, 1000);

	}

    private getServerProcessStatus(): boolean {
        if (this.controller === null) {
            return false;
        }
        return this.controller.is_running();
    }

    private postMessage(m: Message) {
        if (this._view) {
			this._view.webview.postMessage(m);
		}
    }

	public setServerProcessStatus() {
		this.postMessage(new setServerProcessStatusMessage(this.getServerProcessStatus()));
	}

    public setBranch(branch_name: string) {
        this.postMessage(new setBranchMessage(branch_name));
    }

    public updateBranch() {
        this.gitController.current_branch()
            .then(branch => {
                if (branch === undefined || branch === null) {
                    return;
                }
                if (branch === 'master') {
                    return;
                }
                this.setBranch(branch);
            });
    }

	private _getHtmlForWebview(webview: vscode.Webview) {
		// Get the local path to main script run in the webview, then convert it to a uri we can use in the webview.
		const scriptUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'out', 'general.js'));

		// Do the same for the stylesheet.
		const styleResetUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'media', 'reset.css'));
		const styleVSCodeUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'media', 'vscode.css'));
		const styleMainUri = webview.asWebviewUri(vscode.Uri.joinPath(this._extensionUri, 'media', 'general.css'));

		// Use a nonce to only allow a specific script to be run.
		const nonce = getNonce();

		return `<!DOCTYPE html>
			<html lang="en">
			<head>
				<meta charset="UTF-8">

				<!--
					Use a content security policy to only allow loading styles from our extension directory,
					and only allow scripts that have a specific nonce.
					(See the 'webview-sample' extension sample for img-src content security policy examples)
				-->
				<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src ${webview.cspSource}; script-src 'nonce-${nonce}';">

				<meta name="viewport" content="width=device-width, initial-scale=1.0">

				<link href="${styleResetUri}" rel="stylesheet">
				<link href="${styleVSCodeUri}" rel="stylesheet">
				<link href="${styleMainUri}" rel="stylesheet">

				<title>Cat Colors</title>
			</head>
			<body>
                <div>
                    Status: <span id="status">undefined</span>
                </div>
				
                <div>
                    <input placeholder="short description" id="branch-name"></input>
                    <textarea placeholder="elaborate description of this change" id="commit-message"></textarea>
                    <button id="save-button" class="" disabled>Save</button>
                </div>

				<script type="module" nonce="${nonce}" src="${scriptUri}"></script>
			</body>
			</html>`;
	}
}

function getNonce() {
	let text = '';
	const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
	for (let i = 0; i < 32; i++) {
		text += possible.charAt(Math.floor(Math.random() * possible.length));
	}
	return text;
}

