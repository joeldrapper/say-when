// say-when's API in TypeScript: the library's own, as the crate has it
// (see ../../src/lib.rs), with its names in camel case, days and times as
// text, and where a word is as an index into the string it's in.

import { ask } from './module.ts';

// What's given and taken back.

/** A day, written `YYYY-MM-DD`, as every day read is given back. */
export type Day = `${number}-${number}-${number}`;

/** A time of day, written `HH:MM` on the 24-hour clock, as every time read is given back. */
export type Time = `${number}:${number}`;

/**
 * A day, as it can be given: written `YYYY-MM-DD`, or anything with a
 * `year`, `month` (from 1) and `day`, like a `Temporal.PlainDate`, or a
 * `Date`, which is the day it is there in the time zone the code runs in.
 */
export type DayLike = string | Date | { year: number; month: number; day: number };

/**
 * A time of day, as it can be given: written `HH:MM`, or anything with an
 * `hour` and a `minute`, like a `Temporal.PlainTime`.
 */
export type TimeLike = string | { hour: number; minute: number };

/**
 * Where English is being read and written. It decides the order of a date
 * in figures ("3/4" is 3 April in the UK and 4 March in the US, and both
 * in Canada), the day a week starts on, and how a day is labelled.
 */
export type Locale = 'uk' | 'us' | 'ireland' | 'australia' | 'new-zealand' | 'canada';

export type Weekday =
	| 'monday'
	| 'tuesday'
	| 'wednesday'
	| 'thursday'
	| 'friday'
	| 'saturday'
	| 'sunday';

/** What a reader's told, which only the caller knows. */
export interface ReaderOptions {
	/** Where it's reading: the UK unless said. */
	locale?: Locale | undefined;
	/** The days that are off, the days that have names, and the days of the week that are worked. */
	calendar?: Calendar | undefined;
	/** Days that come round every year with names only the caller knows, like birthdays. */
	namedDates?: readonly NamedDate[] | undefined;
	/** The names time zones go by, for an event's sentence to say which its times are in. */
	zones?: ZoneNames | undefined;
}

/**
 * The days that are off and the days that have names, as the caller knows
 * them, and the days of the week that are worked.
 *
 * The library has no country's public holidays, as governments add and
 * move them: give it a list kept from a source you trust. With none, a
 * working day is Monday to Friday, and only the days the same everywhere
 * every year have names, like Christmas and Halloween.
 */
export interface Calendar {
	/** The days off, like public holidays, in any order. */
	daysOff?: readonly DayOff[] | undefined;
	/**
	 * The days with names beyond those built in, like Easter, Mother's Day or
	 * Thanksgiving: each day a name falls on, for every year it can be said
	 * in. A day off that's said like this as well is given in both.
	 */
	namedDays?: readonly NamedDay[] | undefined;
	/** The days of the week that are worked: Monday to Friday unless said. */
	workingWeek?: readonly Weekday[] | undefined;
}

/** A day that's off, like a public holiday. */
export interface DayOff {
	day: DayLike;
	/**
	 * What it's called, like "Summer bank holiday", for it to be said: "the
	 * summer bank holiday", "the august bank holiday".
	 */
	name?: string | null | undefined;
}

/** A day that's called something, one of the times it's on, like "Mother's Day" on 14 March 2027. */
export interface NamedDay {
	/** The day, or for one that lasts longer, like Ramadan or half term, its first. */
	day: DayLike;
	/** Its last day, for one that lasts more than one. */
	last?: DayLike | null | undefined;
	/** What it's called, the first as it's written back: `["Mother's Day", "Mothering Sunday"]`. */
	names: readonly string[];
}

/**
 * A day that comes round every year and has a name only the caller knows,
 * like someone's birthday: "mum's birthday", "a week before Ann's
 * birthday", "our anniversary next year".
 */
export interface NamedDate {
	/**
	 * Each way of saying whose it is: "ann", "ann smith", "mum", "our". With
	 * none, the day is said by what it is alone, like "payday".
	 */
	names: readonly string[];
	/** Each word for what it is: "birthday", "bday". */
	what: readonly string[];
	/** Its month, from 1 for January. */
	month: number;
	/** Its day of the month. The 29th of February is the 28th in a year without one. */
	day: number;
	/** How to say whose day it is, like "Eli Jones's birthday", for what's read from it to say so. */
	label?: string | null | undefined;
}

/**
 * Names, and the time zone each names, as a record or as pairs. Each name
 * is in lower case, without accents, with single spaces, like `'sao paulo'`,
 * and it's read however it's typed: "São Paulo", "sao_paulo".
 */
export type Names = Readonly<Record<string, string>> | Iterable<readonly [string, string]>;

/**
 * The names time zones go by. The reader knows no time zones itself: it's
 * given the names, and gives back whatever each stands for, like
 * `"America/New_York"`.
 */
export interface ZoneNames {
	/** Names read before the word "time", as in "9am New York time". */
	beforeWordTime?: Names | undefined;
	/**
	 * Names read straight after a time, as in "10:00 PT" or "at 3pm Tokyo".
	 * Fewer than the others, since many cities are people's names too.
	 */
	afterATime?: Names | undefined;
}

/** A day a phrase might mean. */
export interface DayReading {
	day: Day;
	/**
	 * How it was read, like "the day before Mon 30 Nov, the last working day
	 * of November 2026", or `null` for a day on its own.
	 */
	how: string | null;
	/** Whose day it was read from, like "2 weeks before Eli Jones's birthday". */
	about: string | null;
}

export type SpanKind = 'week' | 'weekend' | 'month' | 'quarter' | 'year' | 'named';

/** A span of days a phrase names. */
export interface SpanReading {
	first: Day;
	last: Day;
	kind: SpanKind;
}

/** A day what's typed in a date picker's search might mean. */
export interface DaySuggestion {
	day: Day;
	/** What to call it, like "Friday" or "In 3 days", or `null` when the day's own label says it best. */
	name: string | null;
	/** Whose day it was worked out from, like "2 weeks before Eli Jones's birthday". */
	about: string | null;
}

/**
 * How something repeats: on a schedule, from the first day it's on, or so
 * long after it's last done, counted from a day.
 */
export interface Repeat {
	kind: 'schedule' | 'after';
	/** An iCalendar RRULE, like `FREQ=WEEKLY;INTERVAL=2;BYDAY=TH`. */
	rule: string;
	/** The first day it's on, or for one after it's done, the day it's counted from. */
	starts: Day;
	/** The days it doesn't come round on after all, as iCalendar's EXDATE has them. */
	skipped: Day[];
}

/** How something repeats, as it can be given. */
export interface RepeatLike {
	kind: 'schedule' | 'after';
	rule: string;
	starts: DayLike;
	skipped?: readonly DayLike[] | undefined;
}

/** A way a phrase might say something repeats. */
export interface RepeatReading {
	repeat: Repeat;
	/** How it was read, like "Every 2 weeks on Tuesday". */
	how: string;
	/**
	 * The next four days it comes round on ({@link Reader.nextDays} gives
	 * more), or for one after it's done, the day it would next if it were
	 * done today.
	 */
	next: Day[];
}

/**
 * Where something is from a day, like a reminder from its event, written
 * as the library writes it: `-P3D` is 3 days before, `-P1D@09:00` the day
 * before at 9am. It keeps what was said, not the day it lands on, so it
 * follows its event when the event moves.
 */
export type Offset = string;

/** Someone or something mentioned in an event's sentence, however the caller marks them. */
export interface Mention {
	/** As it's written in the sentence, like "#Priya Raman". Matched whatever its case, and with "'s" after it or not. */
	written: string;
	/** As the title says it, like "Priya Raman". */
	said: string;
}

/** What an event's sentence is read with. */
export interface EventContext {
	/** The day it is. */
	today: DayLike;
	/** The time it's at if nothing says. */
	fallbackTime: TimeLike;
	/** The day it's on if nothing says: today unless said. */
	fallbackDay?: DayLike | undefined;
	/** How long it lasts, in minutes, when it's at a time and nothing says for how long: an hour unless said. */
	defaultLength?: number | undefined;
	/** Who's mentioned, in the order they're in the sentence. */
	mentions?: readonly Mention[] | undefined;
}

/**
 * What a word was read as. A word that leads to what it's part of, like
 * "on" in "on friday" or "at" before a place, is read as what it leads to.
 */
export type WordRole =
	| 'title'
	| 'mention'
	| 'day'
	| 'time'
	| 'length'
	| 'repeat'
	| 'timeZone'
	| 'place';

/**
 * A word of what was typed, and what it was read as, for showing which
 * words said what as they're typed. `text.slice(word.start, word.end)` is
 * the word.
 */
export type ReadWord =
	| { start: number; end: number; role: Exclude<WordRole, 'mention'> }
	/** One of the mentions given, by its place among them, all of its words as one. */
	| { start: number; end: number; role: 'mention'; mention: number };

/** A meal, which says when something is when nothing else does: "Lunch with Priya" is at noon. */
export type Meal = 'breakfast' | 'brunch' | 'lunch' | 'drinks' | 'dinner';

/** An event, as its sentence says it. */
export interface EventReading {
	/** What it's called: what's left of the sentence. */
	title: string;
	/** Which of the mentions given were found, by their place among them, in the order they're in. */
	mentioned: number[];
	startDay: Day;
	endDay: Day;
	/** `null` over whole days. */
	startTime: Time | null;
	/** `null` over whole days. */
	endTime: Time | null;
	/** Where it is, if it's said. */
	location: string | null;
	/** How it repeats, from the day it starts, and how that was read. */
	repeat: (Repeat & { how: string }) | null;
	/** The time zone its times are in, when one's named. */
	timeZone: string | null;
	/** What was read, rather than taken as it is when nothing says. */
	read: { day: boolean; time: boolean; length: boolean; meal: Meal | null };
	/** Other days what was typed might start on, like the end of the week's Sunday after its Friday. */
	others: Day[];
	/** Each word typed, and what it was read as. */
	words: ReadWord[];
}

/** When something is, said with nothing else, read as an event's sentence would be. */
export interface WhenReading {
	/** The day it starts: today, if only a time is said. */
	startDay: Day;
	endDay: Day;
	/** `null` over whole days. */
	startTime: Time | null;
	/** `null` over whole days, or when it's at a time and not said until when, like "at 3". */
	endTime: Time | null;
	timeZone: string | null;
	others: Day[];
	words: ReadWord[];
}

// How what's given is written for the module, and what it answers read
// back, in the library's own names (see ../../src).

const pad = (number: number, width: number) => String(number).padStart(width, '0');

function dayOf(day: DayLike, what = 'day'): string {
	if (typeof day === 'string') return day;
	if (day instanceof Date) {
		if (Number.isNaN(day.getTime())) throw new RangeError(`say-when: ${what} is an invalid Date`);
		return `${pad(day.getFullYear(), 4)}-${pad(day.getMonth() + 1, 2)}-${pad(day.getDate(), 2)}`;
	}
	if (typeof day === 'object' && day !== null && Number.isInteger(day.year)) {
		return `${pad(day.year, 4)}-${pad(day.month, 2)}-${pad(day.day, 2)}`;
	}
	throw new TypeError(`say-when: ${what} isn't a day`);
}

function timeOf(time: TimeLike, what = 'time'): string {
	if (typeof time === 'string') return time;
	if (typeof time === 'object' && time !== null && Number.isInteger(time.hour)) {
		return `${pad(time.hour, 2)}:${pad(time.minute, 2)}`;
	}
	throw new TypeError(`say-when: ${what} isn't a time`);
}

function pairs(names: Names | undefined): [string, string][] {
	if (!names) return [];
	if (Symbol.iterator in names) return Array.from(names, ([name, zone]) => [name, zone]);
	return Object.entries(names);
}

function repeatOf(repeat: RepeatLike) {
	return {
		kind: repeat.kind,
		rule: repeat.rule,
		starts: dayOf(repeat.starts, 'repeat.starts'),
		skipped: (repeat.skipped ?? []).map((day) => dayOf(day, 'skipped'))
	};
}

type Said = Record<string, unknown>;

type SaidRepeat = { kind: Repeat['kind']; rule: string; starts: Day; skipped?: Day[] };
const readRepeat = ({ kind, rule, starts, skipped = [] }: SaidRepeat): Repeat => ({
	kind,
	rule,
	starts,
	skipped
});

type SaidRepeatReading = { repeat: SaidRepeat; how: string; next: Day[] };
const readRepeatReading = ({ repeat, how, next }: SaidRepeatReading): RepeatReading => ({
	repeat: readRepeat(repeat),
	how,
	next
});

type SaidRole =
	| 'title'
	| 'day'
	| 'time'
	| 'length'
	| 'repeat'
	| 'time_zone'
	| 'place'
	| { mention: number };
type SaidWord = { start: number; end: number; role: SaidRole };

// The module says where each word is in UTF-8 bytes, as Rust's strings are
// indexed: here they're where they are in the string, in UTF-16.
function readWords(text: string, words: SaidWord[]): ReadWord[] {
	let at: (byte: number) => number = (byte) => byte;
	// eslint-disable-next-line no-control-regex
	if (/[^\u0000-\u007f]/.test(text)) {
		const index = new Map<number, number>([[0, 0]]);
		let bytes = 0;
		let units = 0;
		for (const character of text) {
			const point = character.codePointAt(0) ?? 0;
			bytes += point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x10000 ? 3 : 4;
			units += character.length;
			index.set(bytes, units);
		}
		at = (byte) => index.get(byte) ?? units;
	}
	return words.map(({ start, end, role }) =>
		typeof role === 'object'
			? { start: at(start), end: at(end), role: 'mention', mention: role.mention }
			: { start: at(start), end: at(end), role: role === 'time_zone' ? 'timeZone' : role }
	);
}

type SaidWhen = {
	start_day: Day;
	end_day: Day;
	start_time: Time | null;
	end_time: Time | null;
	time_zone: string | null;
	others: Day[];
	words: SaidWord[];
};

type SaidEvent = SaidWhen & {
	title: string;
	mentioned: number[];
	location: string | null;
	repeat: (SaidRepeat & { how: string }) | null;
	read: EventReading['read'];
};

const readWhen = (text: string, when: SaidWhen): WhenReading => ({
	startDay: when.start_day,
	endDay: when.end_day,
	startTime: when.start_time,
	endTime: when.end_time,
	timeZone: when.time_zone,
	others: when.others,
	words: readWords(text, when.words)
});

function readEvent(text: string, event: SaidEvent): EventReading {
	const when = readWhen(text, event);
	return {
		title: event.title,
		mentioned: event.mentioned,
		startDay: when.startDay,
		endDay: when.endDay,
		startTime: when.startTime,
		endTime: when.endTime,
		location: event.location,
		repeat: event.repeat && { ...readRepeat(event.repeat), how: event.repeat.how },
		timeZone: when.timeZone,
		read: event.read,
		others: when.others,
		words: when.words
	};
}

// Each reader the module holds is let go of when the Reader for it is.
const held = new FinalizationRegistry<number>((reader) => {
	try {
		ask('drop', { reader });
	} catch {
		// Nothing's lost: the module can only have gone with it.
	}
});

// The reader the next Reader made is, when the module already holds it.
let adopting: { id: number; locale: Locale } | null = null;

/**
 * Reads what's typed, for somewhere and someone: days, spans of days,
 * times, how things repeat, reminders and events, written in plain
 * English.
 *
 * Making one works out what it's told once, so make one and keep it,
 * rather than one a phrase.
 *
 * ```ts
 * const reader = new Reader({ locale: 'uk' });
 * reader.days('the last working day of november', '2026-10-20')[0].day; // '2026-11-30'
 * ```
 */
export class Reader {
	readonly #id: number;
	/** Where it reads. */
	readonly locale: Locale;

	constructor(options: ReaderOptions = {}) {
		// A reader the module already holds, as about() makes.
		if (adopting) {
			this.#id = adopting.id;
			this.locale = adopting.locale;
			adopting = null;
			held.register(this, this.#id);
			return;
		}
		const { locale = 'uk', calendar = {}, namedDates = [], zones = {} } = options;
		const id = ask('reader', {
			locale,
			calendar: {
				days_off: (calendar.daysOff ?? []).map(({ day, name }) => ({
					day: dayOf(day, 'calendar.daysOff day'),
					name: name ?? null
				})),
				named_days: (calendar.namedDays ?? []).map(({ day, last, names }) => ({
					day: dayOf(day, 'calendar.namedDays day'),
					last: last == null ? null : dayOf(last, 'calendar.namedDays last'),
					names
				})),
				working_week: calendar.workingWeek ?? null
			},
			named_dates: namedDates.map(({ names, what, month, day, label }) => ({
				names,
				what,
				month,
				day,
				label: label ?? null
			})),
			zones: {
				before_word_time: pairs(zones.beforeWordTime),
				after_a_time: pairs(zones.afterATime)
			}
		}) as number;
		this.#id = id;
		this.locale = locale;
		held.register(this, id);
	}

	#ask(name: string, asked: Said = {}): unknown {
		return ask(name, { reader: this.#id, ...asked });
	}

	/**
	 * A reader that also reads "his", "her" and "their" as whoever some text
	 * names, like the title of the thing a day's being picked for: about
	 * "Plan Eli's party", "2 weeks before his birthday" is before Eli's.
	 */
	about(text: string): Reader {
		adopting = { id: this.#ask('about', { text }) as number, locale: this.locale };
		return new Reader();
	}

	/**
	 * Every day a phrase might mean, by every reading that uses all its
	 * words, the likeliest first: "tomorrow", "next fri", "9 dec", "3
	 * working days before christmas", "mum's birthday".
	 */
	days(text: string, today: DayLike): DayReading[] {
		return this.#ask('days', { text, today: dayOf(today, 'today') }) as DayReading[];
	}

	/** The span of days a phrase names, if it's one, like "next week", "the weekend", "q3 2027" or "november". */
	span(text: string, today: DayLike): SpanReading | null {
		return this.#ask('span', { text, today: dayOf(today, 'today') }) as SpanReading | null;
	}

	/**
	 * The days what's typed in a date picker's search might mean, the
	 * likeliest first, as it's typed: "t" is Today, Tomorrow, Tuesday and
	 * Thursday. Never a day before today.
	 */
	suggestDays(typed: string, today: DayLike): DaySuggestion[] {
		return this.#ask('suggest_days', {
			text: typed,
			today: dayOf(today, 'today')
		}) as DaySuggestion[];
	}

	/**
	 * What's typed with each word that's one letter off a word dates are said
	 * with put right, like "tmorrow" for tomorrow, for searching again when
	 * nothing was found, and the words in `more` put right too. `null` when
	 * no word is.
	 */
	corrected(typed: string, more: readonly string[] = []): string | null {
		return this.#ask('corrected', { text: typed, more }) as string | null;
	}

	/**
	 * Every way a phrase might say something repeats: "every other
	 * thursday", "the last working day of the month", "weekly until
	 * christmas". `from` is the day the thing is on now, which "every week"
	 * and "monthly" take their days from.
	 */
	repeats(text: string, from: DayLike, today: DayLike): RepeatReading[] {
		const said = this.#ask('repeats', {
			text,
			from: dayOf(from, 'from'),
			today: dayOf(today, 'today')
		}) as SaidRepeatReading[];
		return said.map(readRepeatReading);
	}

	/**
	 * What a picker offers before anything's typed, from the day the thing
	 * is on: every day, weekday, week, month and year as of that day, and a
	 * week after it's done.
	 */
	suggestRepeats(from: DayLike, today: DayLike): RepeatReading[] {
		const said = this.#ask('suggest_repeats', {
			from: dayOf(from, 'from'),
			today: dayOf(today, 'today')
		}) as SaidRepeatReading[];
		return said.map(readRepeatReading);
	}

	/**
	 * How something repeats, as people say it, like "Every 2 weeks on
	 * Tuesday" or "3 weeks after it's done". The year of `today` is left off
	 * the day it ends.
	 */
	describeRepeat(repeat: RepeatLike, today?: DayLike): string {
		return this.#ask('describe_repeat', {
			repeat: repeatOf(repeat),
			today: today === undefined ? null : dayOf(today, 'today')
		}) as string;
	}

	/**
	 * The next few days a repeat comes round on, from a day on, or for one
	 * after it's done, the day it would be if it were done that day.
	 */
	nextDays(repeat: RepeatLike, from: DayLike, count: number): Day[] {
		return this.#ask('next_days', {
			repeat: repeatOf(repeat),
			from: dayOf(from, 'from'),
			many: count
		}) as Day[];
	}

	/**
	 * Each day a repeat on a schedule comes round on from one day to another,
	 * both included, in order, but for those it skips, up to `limit` of them.
	 * One after it's done has no days of its own.
	 */
	repeatDays(
		repeat: RepeatLike,
		from: DayLike,
		to: DayLike,
		options: { limit?: number | undefined } = {}
	): Day[] {
		return this.#ask('repeat_days', {
			repeat: repeatOf(repeat),
			from: dayOf(from, 'from'),
			to: dayOf(to, 'to'),
			most: options.limit ?? null
		}) as Day[];
	}

	/** Whether a repeat on a schedule comes round on a day. */
	repeatsOn(repeat: RepeatLike, day: DayLike): boolean {
		return this.#ask('repeats_on', { repeat: repeatOf(repeat), day: dayOf(day) }) as boolean;
	}

	/** The last day a repeat on a schedule comes round on, if it stops, after so many times or on a day. */
	lastRepeatDay(repeat: RepeatLike): Day | null {
		return this.#ask('last_repeat_day', { repeat: repeatOf(repeat) }) as Day | null;
	}

	/** Whether a day is worked, by the reader's calendar. */
	isWorkingDay(day: DayLike): boolean {
		return this.#ask('is_working_day', { day: dayOf(day) }) as boolean;
	}

	/**
	 * A time of day typed on its own: "3pm", "13:30", "half three", "noon".
	 * A bare hour is in the afternoon up to 7, or the first after `after`
	 * when it's given.
	 */
	time(text: string, after?: TimeLike): Time | null {
		return readClock(text, after);
	}

	/**
	 * Where something is from a day, said in words, like a reminder from its
	 * event: "3 days before", "2 working days before", "the friday before",
	 * "the day before at 9am", "an hour before". It says which way, so "3
	 * days" alone is nothing.
	 */
	offset(text: string): Offset | null {
		return this.#ask('offset', { text }) as Offset | null;
	}

	/**
	 * Where an offset lands from a day, and the time on it if there's one: at
	 * the time it says, or the one it's from. Working days are counted by
	 * this reader's calendar. A length of time within a day, like an hour,
	 * lands nowhere from a day with no time.
	 */
	offsetFrom(
		offset: Offset,
		day: DayLike,
		time?: TimeLike
	): { day: Day; time: Time | null } | null {
		const landed = this.#ask('offset_from', {
			offset,
			day: dayOf(day),
			time: time === undefined ? null : timeOf(time)
		}) as [Day, Time | null] | null;
		return landed && { day: landed[0], time: landed[1] };
	}

	/**
	 * Reads an event typed out as a sentence, like "Lunch with Priya
	 * tomorrow at 1 at Monmouth": when it is, how it repeats, where it is,
	 * who's mentioned, and the title that's left, with what each word was
	 * read as.
	 */
	event(text: string, context: EventContext): EventReading {
		const said = this.#ask('event', {
			text,
			context: {
				today: dayOf(context.today, 'context.today'),
				fallback_time: timeOf(context.fallbackTime, 'context.fallbackTime'),
				fallback_day:
					context.fallbackDay === undefined
						? null
						: dayOf(context.fallbackDay, 'context.fallbackDay'),
				default_length: context.defaultLength ?? null,
				mentions: (context.mentions ?? []).map(({ written, said }) => ({ written, said }))
			}
		}) as SaidEvent;
		return readEvent(text, said);
	}

	/**
	 * When something is, typed with nothing else, like "tomorrow 2-4pm", "fri
	 * 6pm to sun 5pm", "next week" or "3pm for an hour", read as an event's
	 * sentence would be. `null` when anything typed isn't when, like "lunch
	 * tomorrow", or it says nothing of when it starts.
	 */
	when(text: string, today: DayLike): WhenReading | null {
		const said = this.#ask('when', { text, today: dayOf(today, 'today') }) as SaidWhen | null;
		return said && readWhen(text, said);
	}

	/**
	 * A day as a list shows it here, like "Tue 20 Oct" or "Tue, Oct 20",
	 * with its year when that isn't the year it is today.
	 */
	labelDay(day: DayLike, today: DayLike): string {
		return this.#ask('label', { day: dayOf(day), today: dayOf(today, 'today') }) as string;
	}

	/**
	 * A day said in full here, like "9 December" or "December 9", with its
	 * year when that isn't the year it is today, or always with no today.
	 */
	longDay(day: DayLike, today?: DayLike): string {
		return this.#ask('long', {
			day: dayOf(day),
			today: today === undefined ? null : dayOf(today, 'today')
		}) as string;
	}

	/** A day without its weekday here, like "17 Nov", with its year when that isn't the year it is today. */
	shortDay(day: DayLike, today: DayLike): string {
		return this.#ask('short', { day: dayOf(day), today: dayOf(today, 'today') }) as string;
	}
}

/**
 * A time of day typed on its own: "3pm", "13:30", "half three", "quarter
 * to 5", "noon". A bare hour is in the afternoon up to 7, or the first
 * after `after` when it's given.
 */
export function readClock(text: string, after?: TimeLike): Time | null {
	return ask('clock', {
		text,
		after: after === undefined ? null : timeOf(after, 'after')
	}) as Time | null;
}

/** How an offset is said, like "the day before at 9am" for `-P1D@09:00`. */
export function describeOffset(offset: Offset): string {
	return ask('describe_offset', { offset }) as string;
}

/** Easter Sunday of a year, for the days that hang on it, or `null` outside the years 1 to 9999. */
export function easter(year: number): Day | null {
	return ask('easter', { year }) as Day | null;
}
