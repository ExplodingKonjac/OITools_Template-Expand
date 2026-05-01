import * as vscode from 'vscode';
import * as path from 'path';
import { expandWithProcess } from './wasm';

type OutputMode = 'clipboard' | 'newFile';
type ExpansionMode = 'default' | 'clipboard' | 'newFile';

// ── Activation ────────────────────────────────────────────────────────────────

export function activate(context: vscode.ExtensionContext) {
    const statusBarItem = vscode.window.createStatusBarItem(
        vscode.StatusBarAlignment.Right, 100,
    );
    statusBarItem.text = '$(beaker) Texpand';
    statusBarItem.tooltip = 'Click to configure Texpand settings';
    statusBarItem.command = 'texpand.showConfigQuickPick';

    const commands = [
        vscode.commands.registerCommand('texpand.expandDefault', () =>
            runExpansion(context, 'default')),
        vscode.commands.registerCommand('texpand.expandAndCopy', () =>
            runExpansion(context, 'clipboard')),
        vscode.commands.registerCommand('texpand.expandToNewFile', () =>
            runExpansion(context, 'newFile')),
        vscode.commands.registerCommand('texpand.showConfigQuickPick', () =>
            showConfigQuickPick()),
    ];

    const visibilityListener = vscode.window.onDidChangeActiveTextEditor(() => {
        updateStatusBar(statusBarItem);
    });

    context.subscriptions.push(statusBarItem, ...commands, visibilityListener);
    setTimeout(() => updateStatusBar(statusBarItem), 0);
}

export function deactivate() {
    // nothing to clean up
}

// ── Status bar visibility ─────────────────────────────────────────────────────

function updateStatusBar(item: vscode.StatusBarItem): void {
    const editor = vscode.window.activeTextEditor;
    if (editor && (editor.document.languageId === 'c' || editor.document.languageId === 'cpp')) {
        item.show();
    } else {
        item.hide();
    }
}

// ── Expansion pipeline ────────────────────────────────────────────────────────

async function runExpansion(context: vscode.ExtensionContext, mode: ExpansionMode): Promise<void> {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
        vscode.window.showErrorMessage('Texpand: No active editor');
        return;
    }

    const langId = editor.document.languageId;
    if (langId !== 'c' && langId !== 'cpp') {
        vscode.window.showErrorMessage('Texpand: Only C/C++ files are supported');
        return;
    }

    const config = vscode.workspace.getConfiguration('texpand');
    const compress = config.get<boolean>('defaultCompression', false);
    const outputMode = config.get<OutputMode>('outputMode', 'clipboard');
    const includePaths = config.get<string[]>('includePaths', ['./']);

    const actualMode: OutputMode = mode === 'default' ? outputMode : mode;
    const entryPath = editor.document.uri.fsPath;

    try {
        const result = await vscode.window.withProgress(
            { location: vscode.ProgressLocation.Notification, title: 'Texpand: Expanding...' },
            () => expandWithProcess(context, entryPath, { compress, includePaths }),
        );

        if (actualMode === 'clipboard') {
            await vscode.env.clipboard.writeText(result);
            vscode.window.showInformationMessage('Texpand: Expanded code copied to clipboard');
        } else {
            const ext = path.extname(entryPath);
            const base = entryPath.slice(0, -ext.length);
            const newFilePath = `${base}.expanded${ext}`;
            const newUri = vscode.Uri.file(newFilePath);
            await vscode.workspace.fs.writeFile(newUri, Buffer.from(result, 'utf-8'));

            const action = await vscode.window.showInformationMessage(
                `Texpand: Expanded to ${path.basename(newFilePath)}`,
                'Open',
            );
            if (action === 'Open') {
                const doc = await vscode.workspace.openTextDocument(newUri);
                await vscode.window.showTextDocument(doc);
            }
        }
    } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes('circular dependency')) {
            vscode.window.showErrorMessage(`Texpand: Circular dependency detected — ${msg}`);
        } else {
            vscode.window.showErrorMessage(`Texpand: ${msg}`);
        }
    }
}

// ── Configuration QuickPick ───────────────────────────────────────────────────

async function showConfigQuickPick(): Promise<void> {
    const config = vscode.workspace.getConfiguration('texpand');
    const compression = config.get<boolean>('defaultCompression', false);
    const outputMode = config.get<string>('outputMode', 'clipboard');
    const includePaths = config.get<string[]>('includePaths', ['./']);

    const items: vscode.QuickPickItem[] = [
        {
            label: `Compression: ${compression ? 'On' : 'Off'}`,
            description: compression ? 'Currently ON — code will be minified' : 'Currently OFF — code will be formatted normally',
        },
        {
            label: `Output Mode: ${outputMode === 'clipboard' ? 'Clipboard' : 'New File'}`,
            description: outputMode === 'clipboard' ? 'Currently: Copy to clipboard' : 'Currently: Write to .expanded.cpp file',
        },
        {
            label: `Include Paths: ${includePaths.length} path(s)`,
            description: includePaths.join(', '),
        },
    ];

    const pick = await vscode.window.showQuickPick(items, {
        placeHolder: 'Select a Texpand setting to change',
    });

    if (!pick) return;

    const target = vscode.ConfigurationTarget.Global;

    if (pick === items[0]) {
        await config.update('defaultCompression', !compression, target);
        vscode.window.showInformationMessage(
            `Texpand: Compression set to ${!compression ? 'On' : 'Off'}`,
        );
    } else if (pick === items[1]) {
        const next = outputMode === 'clipboard' ? 'newFile' : 'clipboard';
        await config.update('outputMode', next, target);
        vscode.window.showInformationMessage(
            `Texpand: Output mode set to ${next === 'clipboard' ? 'Clipboard' : 'New File'}`,
        );
    } else if (pick === items[2]) {
        await vscode.commands.executeCommand(
            'workbench.action.openSettings',
            '@ext:texpand-vscode texpand.includePaths',
        );
    }
}
