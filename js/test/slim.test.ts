// say-when/slim, in a process of its own (as node --test runs each file),
// so the module isn't already there from say-when.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { init, Reader } from '../src/slim.ts';

test('needs init() before reading anything', () => {
	assert.throws(() => new Reader(), /await init\(\)/);
});

test('loads the module from beside it', async () => {
	await Promise.all([init(), init()]);
	assert.equal(new Reader().days('tomorrow', '2026-11-16')[0]?.day, '2026-11-17');
});
