// Builds the package: the library to WebAssembly (see ../wasm), held both
// as a file, for say-when/slim, and inlined in the code, for say-when;
// then the TypeScript, to dist. With --wasm, only the module, as the tests
// need it.
//
// It needs cargo, with the wasm32-unknown-unknown target, which `mise
// install` sets up from the repository's mise.toml.

import { execFileSync } from 'node:child_process';
import { copyFileSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = (path: string) => fileURLToPath(new URL(`../${path}`, import.meta.url));
const run = (command: string, args: string[], cwd = here('')) =>
	execFileSync(command, args, { cwd, stdio: 'inherit' });

const target = 'wasm32-unknown-unknown';
run(
	'cargo',
	['build', '--locked', '-p', 'say-when-wasm', '--profile', 'wasm', '--target', target],
	here('..')
);
const built = here(`../target/${target}/wasm/say_when_wasm.wasm`);
const wasm = readFileSync(built);

copyFileSync(built, here('src/say-when.wasm'));
writeFileSync(
	here('src/inline.ts'),
	'// The WebAssembly module, written by scripts/build.ts from ../wasm. Not committed.\n' +
		`export const wasm: string = '${wasm.toString('base64')}';\n`
);
console.log(`say-when.wasm: ${(wasm.length / 1024).toFixed(0)} KiB`);

if (!process.argv.includes('--wasm')) {
	rmSync(here('dist'), { recursive: true, force: true });
	run('node', [here('node_modules/typescript/bin/tsc'), '-p', 'tsconfig.build.json']);
	// TypeScript writes .js for what's imported as .ts in the code, but not
	// in the types it writes beside it.
	for (const file of readdirSync(here('dist')).filter((name) => name.endsWith('.d.ts'))) {
		const types = readFileSync(here(`dist/${file}`), 'utf8');
		writeFileSync(here(`dist/${file}`), types.replace(/(from '\.\/[\w-]+)\.ts'/g, "$1.js'"));
	}
	copyFileSync(built, here('dist/say-when.wasm'));
	// The licence is the repository's, and npm only packs what's beside it.
	copyFileSync(here('../LICENSE'), here('LICENSE'));
}
