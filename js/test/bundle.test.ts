// The package as a bundler sees it, from dist: what's imported from
// say-when has its module with it, however much is shaken out.

import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, symlinkSync, writeFileSync, mkdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const root = fileURLToPath(new URL('..', import.meta.url));

test('a bundle of say-when reads, with nothing set up', async () => {
	const app = mkdtempSync(join(tmpdir(), 'say-when-'));
	try {
		mkdirSync(join(app, 'node_modules'));
		symlinkSync(root, join(app, 'node_modules', 'say-when'));
		writeFileSync(
			join(app, 'app.js'),
			"import { Reader } from 'say-when';\nexport const day = new Reader().days('tomorrow', '2026-11-16')[0].day;\n"
		);
		const { outputFiles } = await build({
			entryPoints: [join(app, 'app.js')],
			absWorkingDir: app,
			bundle: true,
			format: 'esm',
			platform: 'neutral',
			write: false
		});
		const bundle = outputFiles[0]!.text;
		const url = `data:text/javascript;base64,${Buffer.from(bundle).toString('base64')}`;
		const { day } = (await import(url)) as { day: string };
		assert.equal(day, '2026-11-17');
	} finally {
		rmSync(app, { recursive: true, force: true });
	}
});
