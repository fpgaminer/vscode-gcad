import * as vscode from 'vscode';
import { currentGcadDocument } from './common';
import ToolpathPanel from './panel';

const disposables_: vscode.Disposable[] = [];

let ti: NodeJS.Timeout;


export function activate(context: vscode.ExtensionContext) {
	console.log('Congratulations, your extension "gcad" is now active!');

	let disposable = vscode.commands.registerCommand('gcad.showToolpaths', () => {
		ToolpathPanel.createOrShow(context.extensionUri);
	});
	context.subscriptions.push(disposable);

	if (vscode.window.registerWebviewPanelSerializer) {
		vscode.window.registerWebviewPanelSerializer(ToolpathPanel.viewType, {
			async deserializeWebviewPanel(webviewPanel: vscode.WebviewPanel, state: any) {
				console.log(`Got state: ${state}`);
				webviewPanel.webview.options = ToolpathPanel.getWebviewOptions(context.extensionUri);
				ToolpathPanel.revive(webviewPanel, context.extensionUri);
			}
		});
	}

	vscode.workspace.onDidChangeTextDocument(onDidChangeTextDocument, null, disposables_);
}

export function deactivate() {
	while (disposables_.length) {
		const x = disposables_.pop();
		if (x) {
			x.dispose();
		}
	}
}

function onDidChangeTextDocument(event: vscode.TextDocumentChangeEvent) {
	const current = currentGcadDocument();

	if (current !== event.document) {
		return;
	}

	clearTimeout(ti);
	ti = setTimeout(() => {
		console.log('Will send update...');
		ToolpathPanel.update();
	}, 2000);
}