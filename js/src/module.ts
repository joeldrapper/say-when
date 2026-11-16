// The library as a WebAssembly module (see ../wasm), and how it's asked
// things: a request written into its memory as JSON, and the answer read
// back out of it as JSON. Nothing here is the package's API: reader.ts
// gives the answers their types.

// What the module exports (see ../wasm/src/lib.rs): where to write a
// request of so many bytes, a call that answers it and says how long the
// answer is, and where the answer is.
type Exports = {
	memory: WebAssembly.Memory;
	request(bytes: number): number;
	call(): number;
	answer(): number;
};

/** Where the module can come from, for {@link init}. */
export type ModuleSource =
	| WebAssembly.Module
	| ArrayBuffer
	| ArrayBufferView<ArrayBuffer>
	| Response
	| PromiseLike<Response>
	| URL
	| string;

const encoder = new TextEncoder();
const decoder = new TextDecoder();

let exports: Exports | null = null;
let loading: Promise<void> | null = null;

// Where the module comes from when it isn't given one: from `@milieu/say-when`,
// the bytes in the package itself (see index.ts), compiled the first time
// they're needed, and from `@milieu/say-when/slim`, the file beside it (see
// slim.ts), which init() must load first.
let bytes: (() => ArrayBufferView<ArrayBuffer>) | null = null;
let fallback: (() => ModuleSource) | null = null;

/**
 * Where the module comes from when init() isn't told, for the entries. An
 * app that imports both has the bytes of the one with them.
 */
export function from(where: {
	bytes?: () => ArrayBufferView<ArrayBuffer>;
	source: () => ModuleSource;
}): void {
	bytes ??= where.bytes ?? null;
	if (where.bytes || !fallback) fallback = where.source;
}

// Keeps the first module made, and only that: the readers made are in it,
// by their numbers, so one made after would have none of them.
function hold(instance: WebAssembly.Instance): void {
	if (exports) return;
	const held = instance.exports as Partial<Exports>;
	if (
		!(held.memory instanceof WebAssembly.Memory) ||
		typeof held.request !== 'function' ||
		typeof held.call !== 'function' ||
		typeof held.answer !== 'function'
	) {
		throw new TypeError("say-when: that WebAssembly module isn't say-when's");
	}
	exports = held as Exports;
}

function isResponse(source: unknown): source is Response {
	return typeof Response !== 'undefined' && source instanceof Response;
}

// What an address is relative to: the page, or where Node was started.
function base(): string | undefined {
	if (typeof location !== 'undefined') return location.href;
	const process = (globalThis as { process?: { cwd?(): string } }).process;
	const cwd = process?.cwd?.();
	return cwd === undefined ? undefined : `file://${cwd.endsWith('/') ? cwd : `${cwd}/`}`;
}

// A file's bytes, where there's a file system and the address is of a file.
async function readFile(url: URL): Promise<Uint8Array> {
	const process = (globalThis as { process?: { getBuiltinModule?(name: string): unknown } })
		.process;
	const fs = process?.getBuiltinModule?.('node:fs/promises') as
		| { readFile(path: URL): Promise<Uint8Array> }
		| undefined;
	if (!fs) throw new Error(`say-when: can't read ${url.href} here`);
	return fs.readFile(url);
}

async function instantiate(source: ModuleSource): Promise<WebAssembly.Instance> {
	if (source instanceof WebAssembly.Module) return WebAssembly.instantiate(source, {});
	if (typeof source === 'string' || source instanceof URL) {
		const url = new URL(source, base());
		if (url.protocol === 'file:') return instantiate(await readFile(url));
		return instantiate(fetch(url));
	}
	if (isResponse(source) || (typeof source === 'object' && 'then' in source)) {
		const response = await source;
		if (!response.ok) {
			throw new Error(`say-when: fetching the module failed: ${response.status} ${response.url}`);
		}
		// Compiled as it arrives, where it's served as application/wasm.
		if (
			typeof WebAssembly.instantiateStreaming === 'function' &&
			response.headers.get('Content-Type')?.startsWith('application/wasm')
		) {
			return (await WebAssembly.instantiateStreaming(response, {})).instance;
		}
		return instantiate(await response.arrayBuffer());
	}
	return (await WebAssembly.instantiate(source, {})).instance;
}

/**
 * Compiles the library's WebAssembly module, once, before anything's read.
 *
 * From `@milieu/say-when`, it needn't be called at all: the module's in the
 * package, and it's compiled the first time it's needed. Calling it
 * first compiles it off the main thread, where a browser can.
 *
 * From `@milieu/say-when/slim`, it must be called, and resolve, before anything's
 * read. With no source, it loads `say-when.wasm` from beside the package's
 * code, which bundlers that follow `new URL(…, import.meta.url)` copy
 * over. Or give it the module, its bytes, a response, or where to fetch it
 * from.
 *
 * Called again, it does nothing.
 */
export function init(source?: ModuleSource): Promise<void> {
	if (exports) return Promise.resolve();
	loading ??= (async () => {
		const said = source ?? fallback?.();
		if (said === undefined)
			throw new Error('say-when: init() needs to be told where the module is');
		// Kept unless the module was made while this one was coming.
		hold(await instantiate(said));
	})().catch((error: unknown) => {
		// Asked again, it's tried again, as when the network was down.
		loading = null;
		throw error;
	});
	return loading;
}

function loaded(): Exports {
	if (exports) return exports;
	if (!bytes) {
		throw new Error(
			'say-when: call and await init() before reading anything with @milieu/say-when/slim'
		);
	}
	hold(new WebAssembly.Instance(new WebAssembly.Module(bytes()), {}));
	return exports as unknown as Exports;
}

/** Thrown when the library can't do what it was asked, as for a rule it can't work out. */
export class SayWhenError extends Error {
	override name = 'SayWhenError';
}

// What stopped the module, if it did. It never should: but after a trap,
// what it held may be half changed, so nothing more is asked of it.
let stopped: unknown = null;

/** Asks the module something, and gives back what it answered. */
export function ask(name: string, asked: object): unknown {
	if (stopped) throw new SayWhenError('say-when: the module stopped earlier', { cause: stopped });
	const module = loaded();
	const request = encoder.encode(JSON.stringify({ [name]: asked }));
	let said: string;
	try {
		const at = module.request(request.length);
		// Its memory is looked at afresh each time, as it moves when it grows.
		new Uint8Array(module.memory.buffer, at, request.length).set(request);
		const length = module.call();
		said = decoder.decode(new Uint8Array(module.memory.buffer, module.answer(), length));
	} catch (error) {
		stopped = error;
		throw new SayWhenError(`say-when: the module stopped, asked ${name}`, { cause: error });
	}
	const answer = JSON.parse(said) as { ok: unknown } | { error: string };
	if ('error' in answer) throw new SayWhenError(`say-when: ${answer.error}`);
	return answer.ok;
}
