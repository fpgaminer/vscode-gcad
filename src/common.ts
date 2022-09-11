import * as vscode from 'vscode';


export function currentGcadDocument(): vscode.TextDocument | null {
	const editor = vscode.window.activeTextEditor;
	if (!editor) {
		return null;
	}

	const document = editor.document;
	if (document.languageId !== 'gcad') {
		console.log('currentGcadDocument: Bad language: ', document.languageId);
		return null;
	}

	return document;
}