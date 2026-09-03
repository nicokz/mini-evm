"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
function activate(context) {
    const handler = async (request, _chatContext, stream, token) => {
        const rpcUrl = vscode.workspace
            .getConfiguration('miniEvm')
            .get('rpcUrl', 'http://127.0.0.1:8545/v1/fuzz');
        if (request.command === 'trace') {
            stream.markdown('Opcode tracing is not exposed by the daemon yet. Use `/fuzz` for a full execution pass.');
            return;
        }
        if (request.command !== 'fuzz') {
            stream.markdown('Available commands: `/fuzz` and `/trace`.');
            return;
        }
        const editor = vscode.window.activeTextEditor;
        if (!editor) {
            stream.markdown('No active editor found. Open a file containing hex bytecode.');
            return;
        }
        const text = editor.document.getText(editor.selection) || editor.document.getText();
        const bytecodeHex = extractBytecode(text);
        if (!bytecodeHex) {
            stream.markdown('No valid hex bytecode was found in the active editor.');
            return;
        }
        stream.progress('Sending bytecode to the mini-evm fuzzer...');
        const controller = new AbortController();
        const cancellation = token.onCancellationRequested(() => controller.abort());
        try {
            const response = await fetch(rpcUrl, {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ bytecode_hex: bytecodeHex, iterations: 1_000_000 }),
                signal: controller.signal,
            });
            if (!response.ok) {
                stream.markdown(`RPC server returned ${response.status} ${response.statusText}.`);
                return;
            }
            const result = (await response.json());
            if (result.violation_found) {
                stream.markdown(`### Invariant breach detected\n\nIterations: \`${result.iterations_executed.toLocaleString()}\`\n\nError: \`${result.error_log ?? 'Unknown execution failure'}\`\n\nCounterexample calldata:\n\`\`\`hex\n${result.payload_hex ?? '0x'}\n\`\`\``);
            }
            else {
                stream.markdown(`### Run clean\n\nExecuted ${result.iterations_executed.toLocaleString()} mutations with no invariant failures.`);
            }
        }
        catch (error) {
            if (controller.signal.aborted) {
                stream.markdown('Fuzzing cancelled.');
            }
            else {
                const message = error instanceof Error ? error.message : String(error);
                stream.markdown(`Failed to connect to mini-evm at \`${rpcUrl}\`: ${message}`);
            }
        }
        finally {
            cancellation.dispose();
        }
    };
    const participant = vscode.chat.createChatParticipant('mini-evm.agent', handler);
    context.subscriptions.push(participant);
}
function deactivate() { }
function extractBytecode(text) {
    return text.match(/(?:0x)?[0-9a-fA-F]{20,}/)?.[0];
}
//# sourceMappingURL=extension.js.map