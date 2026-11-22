import { mockIPC } from "@tauri-apps/api/mocks";
import type { Commands } from "$lib/api";

type Handlers = { [K in keyof Commands]?: (args: Commands[K]["args"]) => Commands[K]["result"] | Promise<Commands[K]["result"]> };

export interface IpcCall {
  cmd: string;
  args: unknown;
}

/**
 * Answers Tauri commands from typed handlers. A handler that throws rejects the
 * invoke, like a Rust command returning Err. An unhandled command fails the test.
 * Events are mocked too: `emit` from `@tauri-apps/api/event` reaches `listen`ers.
 */
export function mockCommands(handlers: Handlers): IpcCall[] {
  const calls: IpcCall[] = [];
  mockIPC(
    (cmd, args) => {
      calls.push({ cmd, args });
      const handler = handlers[cmd as keyof Commands] as ((a: unknown) => unknown) | undefined;
      if (!handler) {
        throw new Error(`unmocked command: ${cmd}`);
      }
      return handler(args);
    },
    { shouldMockEvents: true },
  );
  return calls;
}
