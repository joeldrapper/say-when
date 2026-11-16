// Runs the library's own cases (../../tests/cases) through the package, as
// the crate's tests/cases.rs runs them natively: thousands of phrases, each
// with what it should read as. What the package gives back is written as
// the library writes it, and has to be the same, case for case.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
	describeOffset,
	readClock,
	Reader,
	type Locale,
	type NamedDay,
	type ReadWord,
	type Repeat
} from '../src/index.ts';

const directory = new URL('../../tests/cases/', import.meta.url);

type Case = Record<string, unknown> & { text: string; out: unknown };

function cases(name: string): Case[] {
	return readFileSync(new URL(`${name}.jsonl`, directory), 'utf8')
		.split('\n')
		.filter(Boolean)
		.map((line) => JSON.parse(line) as Case);
}

const fixture = JSON.parse(readFileSync(new URL('fixture.json', directory), 'utf8')) as {
	named: { names: string[]; what: string[]; month: number; day: number; label?: string }[];
	zones: { before_word_time: [string, string][]; after_a_time: [string, string][] };
	fallback_time: string;
	calendars: Record<string, { day: string; name: string | null }[]>;
	named_days: Record<string, { names: string[]; days: string[] }[]>;
};

// A reader for each locale, region and what it's about, made once: for
// the UK with England and Wales's holidays unless a case says, anywhere
// else with none unless it says, and with the days with names said in
// the locale.
const readers = new Map<string, Reader>();
function reader(each: Case): Reader {
	const locale = (each['locale'] as Locale | undefined) ?? 'uk';
	const region =
		(each['region'] as string | undefined) ?? (locale === 'uk' ? 'england-and-wales' : undefined);
	const about = each['about'] as string | undefined;
	const key = `${locale} ${region} ${about}`;
	let made = readers.get(key);
	if (!made) {
		if (about) {
			made = reader({ ...each, about: undefined }).about(about);
		} else {
			const namedDays: NamedDay[] = (fixture.named_days[locale] ?? []).flatMap(({ names, days }) =>
				days.map((days) => {
					const [day, last] = days.split('/') as [string, string | undefined];
					return { day, last: last ?? null, names };
				})
			);
			made = new Reader({
				locale,
				calendar: { daysOff: region ? fixture.calendars[region] : [], namedDays },
				namedDates: fixture.named,
				zones: {
					beforeWordTime: fixture.zones.before_word_time,
					afterATime: fixture.zones.after_a_time
				}
			});
		}
		readers.set(key, made);
	}
	return made;
}

// Runs a file of cases, and fails with the first few that read
// differently.
function run(name: string, read: (each: Case, reader: Reader) => unknown) {
	const all = cases(name);
	assert.ok(all.length > 40, `only ${all.length} ${name} cases`);
	const wrong = [];
	for (const each of all) {
		const out = read(each, reader(each));
		try {
			assert.deepStrictEqual(out, each.out);
		} catch {
			wrong.push({ text: each.text, out, should: each.out });
		}
	}
	assert.deepStrictEqual(
		wrong.slice(0, 5),
		[],
		`${wrong.length} of ${all.length} ${name} cases differ`
	);
}

// What's given back, written back as the library writes it.

const said = (repeat: Repeat) => {
	const { skipped, ...rest } = repeat;
	return skipped.length ? { ...rest, skipped } : rest;
};

const bytes = (text: string, at: number) => Buffer.byteLength(text.slice(0, at));
const words = (text: string, words: ReadWord[]) =>
	words.map((word) => ({
		start: bytes(text, word.start),
		end: bytes(text, word.end),
		role:
			word.role === 'mention'
				? { mention: word.mention }
				: word.role === 'timeZone'
					? 'time_zone'
					: word.role
	}));

const today = (each: Case) => each['today'] as string;

test('days and spans', () => {
	run('days', (each, reader) => ({
		days: reader.days(each.text, today(each)),
		span: reader.span(each.text, today(each))
	}));
});

test('suggestions', () => {
	run('search', (each, reader) => ({
		days: reader.suggestDays(each.text, today(each)),
		corrected: reader.corrected(each.text)
	}));
});

test('repeats', () => {
	run('repeats', (each, reader) =>
		reader
			.repeats(each.text, each['from'] as string, today(each))
			.map(({ repeat, how, next }) => ({ repeat: said(repeat), how, next }))
	);
});

test('events', () => {
	run('events', (each, reader) => {
		const mentions = ((each['mentions'] as string[] | undefined) ?? []).map((name) => ({
			written: `#${name}`,
			said: name
		}));
		const event = reader.event(each.text, {
			today: today(each),
			fallbackTime: fixture.fallback_time,
			mentions
		});
		return {
			title: event.title,
			mentioned: event.mentioned,
			start_day: event.startDay,
			end_day: event.endDay,
			start_time: event.startTime,
			end_time: event.endTime,
			location: event.location,
			repeat: event.repeat && { ...said(event.repeat), how: event.repeat.how },
			time_zone: event.timeZone,
			read: event.read,
			others: event.others,
			words: words(each.text, event.words)
		};
	});
});

test('when', () => {
	run('when', (each, reader) => {
		const when = reader.when(each.text, today(each));
		return (
			when && {
				start_day: when.startDay,
				end_day: when.endDay,
				start_time: when.startTime,
				end_time: when.endTime,
				time_zone: when.timeZone,
				others: when.others,
				words: words(each.text, when.words)
			}
		);
	});
});

test('offsets', () => {
	run('offsets', (each, reader) => {
		const offset = reader.offset(each.text);
		if (offset === null) return null;
		// Landed from a Monday at 2pm, and from the same day with no time.
		const landed = (time?: string) => {
			const at = reader.offsetFrom(offset, '2026-11-16', time);
			return at && [at.day, at.time];
		};
		return { offset, said: describeOffset(offset), from: landed('14:00'), all_day: landed() };
	});
});

test('clocks', () => {
	run('clocks', (each) => readClock(each.text, (each['after'] as string | undefined) ?? undefined));
});
