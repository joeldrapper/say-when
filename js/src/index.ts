// say-when, with its WebAssembly module in the package's code, so it's
// read from as soon as it's imported, anywhere JavaScript runs, with
// nothing to set up. The module's compiled the first time it's needed;
// `await init()` compiles it ahead, off the main thread where it can be.
// See slim.ts for the same without the module inlined.

import { wasm } from './inline.ts';
import { from } from './module.ts';

let decoded: Uint8Array<ArrayBuffer> | null = null;

function bytes(): Uint8Array<ArrayBuffer> {
	decoded ??= Uint8Array.from(atob(wasm), (character) => character.charCodeAt(0));
	return decoded;
}

from({ bytes, source: bytes });

export * from './reader.ts';
export { init, SayWhenError, type ModuleSource } from './module.ts';
