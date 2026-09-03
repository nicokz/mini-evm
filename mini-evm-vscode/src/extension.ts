import * as vscode from 'vscode';

interface FuzzResponse {
  status: string;
  iterations_executed: number;
  violation_found: boolean;
  payload_hex?: string;
  error_log?: string;
}

export function activate(context: vscode.ExtensionContext): void {
  const handler: vscode.ChatRequestHandler = async (
    request,
    _chatContext,
    stream,
    token,
  ): Promise<void> => {
    const rpcUrl = vscode.workspace
      .getConfiguration('miniEvm')
      .get<string>('rpcUrl', 'http://127.0.0.1:8545/v1/fuzz');

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

      const result = (await response.json()) as FuzzResponse;
      if (result.violation_found) {
        stream.markdown(`### Invariant breach detected\n\nIterations: \`${result.iterations_executed.toLocaleString()}\`\n\nError: \`${result.error_log ?? 'Unknown execution failure'}\`\n\nCounterexample calldata:\n\`\`\`hex\n${result.payload_hex ?? '0x'}\n\`\`\``);
      } else {
        stream.markdown(`### Run clean\n\nExecuted ${result.iterations_executed.toLocaleString()} mutations with no invariant failures.`);
      }
    } catch (error: unknown) {
      if (controller.signal.aborted) {
        stream.markdown('Fuzzing cancelled.');
      } else {
        const message = error instanceof Error ? error.message : String(error);
        stream.markdown(`Failed to connect to mini-evm at \`${rpcUrl}\`: ${message}`);
      }
    } finally {
      cancellation.dispose();
    }
  };

  const participant = vscode.chat.createChatParticipant('mini-evm.agent', handler);
  context.subscriptions.push(participant);
}

export function deactivate(): void {}

function extractBytecode(text: string): string | undefined {
  return text.match(/(?:0x)?[0-9a-fA-F]{20,}/)?.[0];
}
