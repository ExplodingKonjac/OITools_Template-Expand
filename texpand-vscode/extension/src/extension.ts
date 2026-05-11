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
    statusBarItem.text = vscode.l10n.t('$(file-code) Texpand');
    statusBarItem.tooltip = vscode.l10n.t('Click to configure Texpand settings');
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
        vscode.window.showErrorMessage(vscode.l10n.t('Texpand: No active editor'));
        return;
    }

    const langId = editor.document.languageId;
    if (langId !== 'c' && langId !== 'cpp') {
        vscode.window.showErrorMessage(vscode.l10n.t('Texpand: Only C/C++ files are supported'));
        return;
    }

    const config = vscode.workspace.getConfiguration('texpand');
    const compress = config.get<boolean>('defaultCompression', false);
    const outputMode = config.get<OutputMode>('outputMode', 'clipboard');
    const includePaths = config.get<string[]>('includePaths', ['./']);

    const actualMode: OutputMode = mode === 'default' ? outputMode : mode;
    const entryPath = editor.document.uri.fsPath;

    try {
        if (config.get<boolean>('saveBeforeExpansion', true)) {
            await editor.document.save();
        }

        const result = await vscode.window.withProgress(
            { location: vscode.ProgressLocation.Notification, title: vscode.l10n.t('Texpand: Expanding...') },
            () => expandWithProcess(context, entryPath, { compress, includePaths }),
        );

        if (actualMode === 'clipboard') {
            await vscode.env.clipboard.writeText(result);
            vscode.window.showInformationMessage(vscode.l10n.t('Texpand: Expanded code copied to clipboard'));
        } else {
            const ext = path.extname(entryPath);
            const base = entryPath.slice(0, -ext.length);
            const newFilePath = `${base}.expanded${ext}`;
            const newUri = vscode.Uri.file(newFilePath);
            await vscode.workspace.fs.writeFile(newUri, Buffer.from(result, 'utf-8'));

            const action = await vscode.window.showInformationMessage(
                vscode.l10n.t('Texpand: Expanded to {0}', path.basename(newFilePath)),
                vscode.l10n.t('Open'),
            );
            if (action === 'Open') {
                const doc = await vscode.workspace.openTextDocument(newUri);
                await vscode.window.showTextDocument(doc);
            }
        }
    } catch (e) {
        const msg = e instanceof Error ? e.message : String(e);
        if (msg.includes('circular dependency')) {
            vscode.window.showErrorMessage(vscode.l10n.t('Texpand: Circular dependency detected — {0}', msg));
        } else {
            vscode.window.showErrorMessage(vscode.l10n.t('Texpand: {0}', msg));
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
            label: `$(file-zip) ${vscode.l10n.t(compression ? 'Compression: On' : 'Compression: Off')}`,
            description: compression
                ? vscode.l10n.t('Currently ON — code will be minified')
                : vscode.l10n.t('Currently OFF — code will be formatted normally'),
        },
        {
            label: `$(output) ${vscode.l10n.t(outputMode === 'clipboard' ? 'Output Mode: Clipboard' : 'Output Mode: New File')}`,
            description: outputMode === 'clipboard'
                ? vscode.l10n.t('Currently: Copy to clipboard')
                : vscode.l10n.t('Currently: Write to .expanded.cpp file'),
        },
        {
            label: `$(list-unordered) ${vscode.l10n.t('Include Paths: {0} path(s)', includePaths.length)}`,
            description: includePaths.join(', '),
        },
    ];

    const pick = await vscode.window.showQuickPick(items, {
        placeHolder: vscode.l10n.t('Select a Texpand setting to change'),
    });

    if (!pick) return;

    const target = vscode.ConfigurationTarget.Global;

    if (pick === items[0]) {
        await config.update('defaultCompression', !compression, target);
        vscode.window.showInformationMessage(
            vscode.l10n.t(!compression ? 'Texpand: Compression set to On' : 'Texpand: Compression set to Off'),
        );
    } else if (pick === items[1]) {
        const next = outputMode === 'clipboard' ? 'newFile' : 'clipboard';
        await config.update('outputMode', next, target);
        vscode.window.showInformationMessage(
            vscode.l10n.t(next === 'clipboard' ? 'Texpand: Output mode set to Clipboard' : 'Texpand: Output mode set to New File'),
        );
    } else if (pick === items[2]) {
        await vscode.commands.executeCommand(
            'workbench.action.openSettings',
            '@ext:explodingkonjac.texpand-vscode texpand.includePaths',
        );
    }
}
