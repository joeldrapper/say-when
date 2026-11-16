// What the package adds to the library: days and times given however
// JavaScript holds them, words found by where they are in the string, and
// the module loaded either way.

import assert from 'node:assert/strict';
import { test } from 'node:test';
import { easter, init, Reader, readClock, SayWhenError } from '../src/index.ts';

const today = '2026-11-16';

test('reads as the README says', () => {
	const reader = new Reader();
	const [read] = reader.days('the day before the last working day of november', '2026-10-20');
	assert.equal(read?.day, '2026-11-29');
	assert.equal(read?.how, 'the day before Mon 30 Nov, the last working day of November 2026');

	const [repeat] = reader.repeats(
		'every other thursday until christmas',
		'2026-10-20',
		'2026-10-20'
	);
	assert.equal(repeat?.repeat.rule, 'FREQ=WEEKLY;INTERVAL=2;BYDAY=TH;UNTIL=20261225');
	assert.equal(repeat?.how, 'Every 2 weeks on Thursday, until 25 December');
	assert.deepEqual(reader.nextDays(repeat!.repeat, '2026-11-01', 3), [
		'2026-11-05',
		'2026-11-19',
		'2026-12-03'
	]);
	assert.equal(reader.lastRepeatDay(repeat!.repeat), '2026-12-17');
	assert.equal(reader.repeatsOn(repeat!.repeat, '2026-11-19'), true);
	assert.equal(reader.repeatsOn(repeat!.repeat, '2026-11-12'), false);

	const offset = reader.offset('the day before at 9am');
	assert.equal(offset, '-P1D@09:00');
	assert.deepEqual(reader.offsetFrom(offset!, '2026-12-03', '14:00'), {
		day: '2026-12-02',
		time: '09:00'
	});

	const when = reader.when('fri 6pm to sun 5pm', today);
	assert.deepEqual(
		[when?.startDay, when?.startTime, when?.endDay, when?.endTime],
		['2026-11-20', '18:00', '2026-11-22', '17:00']
	);
	assert.equal(reader.when('lunch tomorrow', today), null);
});

test('reads with what only the caller knows', () => {
	const reader = new Reader({
		locale: 'us',
		calendar: {
			daysOff: [{ day: '2026-11-26', name: 'Thanksgiving' }],
			namedDays: [{ day: easter(2027)!, names: ['Easter Sunday', 'Easter'] }]
		},
		namedDates: [
			{ names: ['mum'], what: ['birthday'], month: 3, day: 14, label: 'Mum’s birthday' }
		],
		zones: { afterATime: new Map([['pt', 'America/Los_Angeles']]) }
	});
	assert.equal(reader.locale, 'us');
	assert.equal(reader.days('2 working days after 11/25', '2026-10-20')[0]?.day, '2026-11-30');
	assert.equal(reader.days('a week before easter', '2026-10-20')[0]?.day, '2027-03-21');
	assert.deepEqual(reader.days("mum's birthday", '2026-10-20'), [
		{ day: '2027-03-14', how: null, about: 'Mum’s birthday' }
	]);
	assert.equal(reader.isWorkingDay('2026-11-26'), false);
	assert.equal(reader.labelDay('2026-11-26', today), 'Thu, Nov 26');
	const call = reader.event('Call 10:00 PT', { today, fallbackTime: '14:00' });
	assert.equal(call.timeZone, 'America/Los_Angeles');

	const fourDays = new Reader({
		calendar: { workingWeek: ['monday', 'tuesday', 'wednesday', 'thursday'] }
	});
	assert.equal(fourDays.isWorkingDay('2026-11-20'), false);
});

test('reads about whoever a title names', () => {
	const reader = new Reader({
		namedDates: [{ names: ['eli'], what: ['birthday'], month: 3, day: 14, label: 'Eli’s birthday' }]
	});
	assert.deepEqual(reader.days('his birthday', today), []);
	const about = reader.about("Plan Eli's party");
	assert.equal(about.locale, 'uk');
	assert.equal(about.days('2 weeks before his birthday', today)[0]?.day, '2027-02-28');
});

test('takes days and times however JavaScript holds them', () => {
	const reader = new Reader();
	const tomorrow = (day: Parameters<Reader['days']>[1]) => reader.days('tomorrow', day)[0]?.day;
	assert.equal(tomorrow('2026-11-16'), '2026-11-17');
	// A Date is the day it is where the code runs.
	assert.equal(tomorrow(new Date(2026, 10, 16, 23, 59)), '2026-11-17');
	// Anything with a year, month and day, like a Temporal.PlainDate.
	assert.equal(tomorrow({ year: 2026, month: 11, day: 16 }), '2026-11-17');
	assert.equal(readClock('9', { hour: 20, minute: 0 }), '21:00');
	assert.equal(readClock('9', '08:00'), '09:00');

	assert.throws(() => tomorrow(new Date(Number.NaN)), RangeError);
	assert.throws(() => tomorrow(20261116 as never), TypeError);
	assert.throws(() => tomorrow('16/11/2026'), SayWhenError);
	assert.throws(
		() => reader.describeRepeat({ kind: 'schedule', rule: 'FREQ=FORTNIGHTLY', starts: today }),
		/the rule isn't one/
	);
});

test('says where each word is in the string', () => {
	const reader = new Reader();
	const text = 'Café with #Zoë 🎉 tomorrow 1-2pm at Dishoom';
	const event = reader.event(text, {
		today,
		fallbackTime: '09:00',
		mentions: [{ written: '#Zoë', said: 'Zoë' }]
	});
	assert.equal(event.title, 'Café with Zoë 🎉');
	assert.deepEqual(
		event.words.map((word) => [text.slice(word.start, word.end), word.role]),
		[
			['Café', 'title'],
			['with', 'title'],
			['#Zoë', 'mention'],
			['🎉', 'title'],
			['tomorrow', 'day'],
			['1-2pm', 'time'],
			['at', 'place'],
			['Dishoom', 'place']
		]
	);
	assert.deepEqual(event.words[2], { start: 10, end: 14, role: 'mention', mention: 0 });

	const zones = new Reader({ zones: { beforeWordTime: { 'sao paulo': 'America/Sao_Paulo' } } });
	const zoned = '3pm São Paulo time';
	const when = zones.when(zoned, today);
	assert.equal(when?.timeZone, 'America/Sao_Paulo');
	assert.deepEqual(
		when?.words.map((word) => [zoned.slice(word.start, word.end), word.role]),
		[
			['3pm', 'time'],
			['São', 'timeZone'],
			['Paulo', 'timeZone'],
			['time', 'timeZone']
		]
	);
});

test('repeats a set number of days at most', () => {
	const reader = new Reader();
	const daily = { kind: 'schedule', rule: 'FREQ=DAILY', starts: '2026-01-01' } as const;
	assert.deepEqual(reader.repeatDays(daily, '2026-01-01', '9999-12-31', { limit: 2 }), [
		'2026-01-01',
		'2026-01-02'
	]);
	const skipping = { ...daily, skipped: ['2026-01-02'] };
	assert.deepEqual(reader.repeatDays(skipping, '2026-01-01', '2026-01-03'), [
		'2026-01-01',
		'2026-01-03'
	]);
	assert.equal(
		reader.describeRepeat({ ...daily, rule: 'FREQ=WEEKLY;INTERVAL=3', kind: 'after' }),
		'3 weeks after it’s done'
	);
});

test('init() resolves at once, as the module is in the package', async () => {
	await init();
	await init();
});
