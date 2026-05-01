import * as vscode from 'vscode';
import { Wasm } from '@vscode/wasm-wasi/v1';

let wasmApi: Wasm | undefined;

async function getWasm(): Promise<Wasm> {
    if (!wasmApi) {
        wasmApi = await Wasm.load();
    }
    return wasmApi;
}

export interface ExpandOptions {
    compress: boolean;
    includePaths: string[];
}

export async function expandWithProcess(
    context: vscode.ExtensionContext,
    entryPath: string,
    opts: ExpandOptions,
): Promise<string> {
    const api = await getWasm();
    const wasmUri = vscode.Uri.joinPath(context.extensionUri, 'pkg', 'texpand-vscode.wasm');
    const module = await api.compile(wasmUri);

    const proc = await api.createProcess('texpand', module, {
        env: {
            TEXPAND_ENTRY_PATH: entryPath,
            TEXPAND_COMPRESS: opts.compress ? 'true' : 'false',
            TEXPAND_INCLUDE_PATHS: opts.includePaths.join(','),
        },
        stdio: {
            out: { kind: 'pipeOut' },
            err: { kind: 'pipeOut' },
        },
        mountPoints: [
            { kind: 'workspaceFolder' },
        ],
    });

    let stdout = '';
    proc.stdout?.onData((data: Uint8Array) => {
        stdout += new TextDecoder().decode(data);
    });

    let stderr = '';
    proc.stderr?.onData((data: Uint8Array) => {
        stderr += new TextDecoder().decode(data);
    });

    const exitCode = await proc.run();

    if (exitCode !== 0) {
        throw new Error(stderr || `Process exited with code ${exitCode}`);
    }

    const result = JSON.parse(stdout.trim());
    if (!result.success) {
        throw new Error(result.error || 'Unknown expansion error');
    }
    return result.data;
}
