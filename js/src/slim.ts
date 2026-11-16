// say-when without its WebAssembly module inlined: the module is
// say-when.wasm beside this, which `await init()` fetches and compiles
// as it arrives, or is told where to find, before anything's read.

import { from } from './module.ts';

from({ source: () => new URL('./say-when.wasm', import.meta.url) });

export * from './reader.ts';
export { init, SayWhenError, type ModuleSource } from './module.ts';
