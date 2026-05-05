import * as fs from 'fs';
import * as vscode from 'vscode';
import * as path from 'path';
import { Wasm } from '@vscode/wasm-wasi/v1';

let wasmApi: Wasm | undefined;
let compiledModule: WebAssembly.Module | undefined;

async function getWasm(): Promise<Wasm> {
    if (!wasmApi) {
        wasmApi = await Wasm.load();
    }
    return wasmApi;
}

async function getModule(api: Wasm, context: vscode.ExtensionContext): Promise<WebAssembly.Module> {
    if (!compiledModule) {
        const wasmUri = vscode.Uri.joinPath(context.extensionUri, 'pkg', 'texpand-vscode.wasm');
        compiledModule = await api.compile(wasmUri);
        console.log('[texpand] WASM module compiled (cached)');
    }
    return compiledModule;
}

export interface ExpandOptions {
    compress: boolean;
    includePaths: string[];
}

/// Convert a host path to a WASI path under `/workspace`.
/// Relative paths are first resolved against the workspace root.
function toWasiPath(hostPath: string, workspaceRoot: string): string {
    const absPath = path.isAbsolute(hostPath)
        ? hostPath
        : path.resolve(workspaceRoot, hostPath);
    const rel = path.relative(workspaceRoot, absPath);
    if (!rel.startsWith('..') && !path.isAbsolute(rel)) {
        return path.posix.join('/workspace', rel);
    }
    return absPath;
}

export async function expandWithProcess(
    context: vscode.ExtensionContext,
    entryPath: string,
    opts: ExpandOptions,
): Promise<string> {
    const api = await getWasm();

    const module = await getModule(api, context);

    const workspaceRoot = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? '/';
    const wasiEntryPath = toWasiPath(entryPath, workspaceRoot);
    const wasiIncludePaths = opts.includePaths.map((p) => toWasiPath(p, workspaceRoot));

    console.log('[texpand] workspaceRoot:', workspaceRoot);
    console.log('[texpand] host entryPath:', entryPath);
    console.log('[texpand] wasi entryPath:', wasiEntryPath);
    console.log('[texpand] includePaths:', wasiIncludePaths);
    console.log('[texpand] compress:', opts.compress);

    // Mount absolute include paths so the WASI process can access them.
    const extraMountPoints: { kind: 'vscodeFileSystem'; uri: vscode.Uri; mountPoint: string }[] = [];
    for (const p of opts.includePaths) {
        if (path.isAbsolute(p) && fs.existsSync(p)) {
            extraMountPoints.push({
                kind: 'vscodeFileSystem',
                uri: vscode.Uri.file(p),
                mountPoint: p,
            });
        }
    }
    if (extraMountPoints.length > 0) {
        console.log('[texpand] extra mount points:', extraMountPoints.map(m => m.mountPoint));
    }

    const proc = await api.createProcess('texpand', module, {
        env: {
            TEXPAND_ENTRY_PATH: wasiEntryPath,
            TEXPAND_COMPRESS: opts.compress ? 'true' : 'false',
            TEXPAND_INCLUDE_PATHS: wasiIncludePaths.join(','),
        },
        stdio: {
            out: { kind: 'pipeOut' },
            err: { kind: 'pipeOut' },
        },
        mountPoints: [
            { kind: 'workspaceFolder' },
            ...extraMountPoints,
        ],
    });

    console.log('[texpand] Process created, running...');

    let stdout = '';
    proc.stdout?.onData((data: Uint8Array) => {
        stdout += new TextDecoder().decode(data);
    });

    let stderr = '';
    proc.stderr?.onData((data: Uint8Array) => {
        stderr += new TextDecoder().decode(data);
    });

    const exitCode = await proc.run();

    console.log('[texpand] exitCode:', exitCode);
    console.log('[texpand] stdout:', stdout.substring(0, 500));
    if (stderr) {
        console.log('[texpand] stderr:', stderr.substring(0, 500));
    }

    if (exitCode !== 0) {
        throw new Error(stderr || `Process exited with code ${exitCode}`);
    }

    const result = JSON.parse(stdout.trim());
    if (!result.success) {
        throw new Error(result.error || 'Unknown expansion error');
    }
    return result.data;
}
