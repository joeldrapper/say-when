// init() still coming when the module's made for a reader, in a process of
// its own, so nothing's made before.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { init, Reader } from '../src/index.ts';

test('a reader made while init() is coming keeps reading as it was told', async () => {
	const coming = init();
	const us = new Reader({ locale: 'us' });
	await coming;
	new Reader();
	assert.equal(us.days('3/4', '2026-01-01')[0]?.day, '2026-03-04');
});
