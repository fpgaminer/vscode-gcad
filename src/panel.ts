import * as vscode from 'vscode';
import * as path from 'path';
import { currentGcadDocument } from './common';


export default class ToolpathPanel {
	public static currentPanel: ToolpathPanel | undefined;

	public static readonly viewType = 'toolpath';

	private readonly _panel: vscode.WebviewPanel;
	private readonly _extensionUri: vscode.Uri;
	private _disposables: vscode.Disposable[] = [];

	private constructor(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
		this._panel = panel;
		this._extensionUri = extensionUri;
		this._panel.onDidDispose(() => this.dispose(), null, this._disposables);

		this._panel.webview.onDidReceiveMessage(
			message => {
				if ('error' in message) {
					vscode.window.showErrorMessage(message.error);
				}
			},
			null,
			this._disposables,
		);

		this.render();
	}

	public static createOrShow(extensionUri: vscode.Uri) {
		const column = vscode.window.activeTextEditor ? vscode.ViewColumn.Beside : vscode.ViewColumn.One;

		if (ToolpathPanel.currentPanel) {
			ToolpathPanel.currentPanel._panel.reveal(column);
			return;
		}

		const panel = vscode.window.createWebviewPanel(
			ToolpathPanel.viewType,
			'Toolpaths',
			{
				viewColumn: column,
				preserveFocus: true,
			},
			ToolpathPanel.getWebviewOptions(extensionUri),
		);

		ToolpathPanel.currentPanel = new ToolpathPanel(panel, extensionUri);
		return ToolpathPanel.currentPanel;
	}

	public static revive(panel: vscode.WebviewPanel, extensionUri: vscode.Uri) {
		ToolpathPanel.currentPanel = new ToolpathPanel(panel, extensionUri);
	}

	public dispose() {
		ToolpathPanel.currentPanel = undefined;

		this._panel.dispose();

		while (this._disposables.length) {
			const x = this._disposables.pop();
			if (x) {
				x.dispose();
			}
		}
	}

	public static getLocalResourceRoots(extensionUri: vscode.Uri): vscode.Uri[] {
		const rootPath = (process.platform === 'win32') ? process.cwd().split(path.sep)[0] : '/';
		const localResourceRoots = [];
		//localResourceRoots.push(vscode.Uri.file(path.join(extensionPath, 'resources')));
		localResourceRoots.push(vscode.Uri.file(rootPath));
		return localResourceRoots;
	}

	public static render() {
		if (ToolpathPanel.currentPanel) {
			ToolpathPanel.currentPanel.render();
		}
	}

	private render() {
		this._panel.title = 'Toolpaths';
		this._panel.webview.html = this.getHtmlForWebview(this._panel.webview);
	}

	private getHtmlForWebview(webview: vscode.Webview): string {
		const scriptPathOnDisk = vscode.Uri.joinPath(this._extensionUri, 'dist', 'app.js');
		const scriptUri = webview.asWebviewUri(scriptPathOnDisk);
		const cssPathOnDisk = vscode.Uri.joinPath(this._extensionUri, 'dist', 'app.css');
		const cssUri = webview.asWebviewUri(cssPathOnDisk);

		const nonce = this.getNonce();

		const content = /* html */`
			<!DOCTYPE html>
			<html lang="en">
			<head>
				<meta charset="UTF-8">
				<meta http-equiv="Content-Security-Policy" content="
					default-src 'none';
					connect-src ${webview.cspSource} https: http: data: blob:;
					style-src ${webview.cspSource};
					img-src ${webview.cspSource} https:;
					script-src 'nonce-${nonce}' 'unsafe-eval';">
				<meta name="viewport" content="width=device-width, initial-scale=1.0">
				<link nonce="${nonce}" href="${cssUri}" rel="stylesheet"/>
			</head>
			<body class="idle">
					<div class="content">
						<canvas id="toolpath-canvas"></canvas>
					</div>
					<script nonce="${nonce}" type="module" src="${scriptUri}"></script>
			</body>
			</html>`;
		return content;
	}

	public static getWebviewOptions(extensionUri: vscode.Uri): vscode.WebviewOptions {
		return {
			enableScripts: true,
			localResourceRoots: this.getLocalResourceRoots(extensionUri),
		};
	}

	private getNonce(): string {
		let text = '';
		const possible = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
		for (let i = 0; i < 32; i++) {
			text += possible.charAt(Math.floor(Math.random() * possible.length));
		}
		return text;
	}

	public static update() {
		if (ToolpathPanel.currentPanel) {
			ToolpathPanel.currentPanel.update();
		}
	}

	public update() {
		const document = currentGcadDocument();
		const code = document?.getText() || '';

		this._panel.webview.postMessage(code);
	}
}