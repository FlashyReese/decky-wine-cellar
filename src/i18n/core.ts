import { FluentBundle, FluentResource } from "@fluent/bundle";
import { ReactLocalization } from "@fluent/react";
import catalogs from "virtual:fluent-catalogs";

export type TranslationKey = string;
export type MessageParams = Record<string, string | number | LocalizedMessage>;
export type LocalizedMessage = {
  key: string;
  params?: MessageParams;
};
export type DisplayMessage = string | LocalizedMessage;
export type Translator = (key: TranslationKey, params?: MessageParams) => string;

// Steam API identifiers and language tags.
// https://partner.steamgames.com/doc/store/localization/languages
export const steamLanguages = {
  arabic: "ar",
  bulgarian: "bg",
  schinese: "zh-CN",
  tchinese: "zh-TW",
  czech: "cs",
  danish: "da",
  dutch: "nl",
  english: "en",
  finnish: "fi",
  french: "fr",
  german: "de",
  greek: "el",
  hungarian: "hu",
  indonesian: "id",
  italian: "it",
  japanese: "ja",
  koreana: "ko",
  malay: "ms",
  norwegian: "no",
  polish: "pl",
  portuguese: "pt",
  brazilian: "pt-BR",
  romanian: "ro",
  russian: "ru",
  spanish: "es",
  latam: "es-419",
  swedish: "sv",
  thai: "th",
  turkish: "tr",
  ukrainian: "uk",
  vietnamese: "vi",
} as const;

export type Locale = keyof typeof steamLanguages;
export const DEFAULT_LOCALE: Locale = "english";

export const locales = Object.fromEntries(
  Object.entries(steamLanguages).map(([locale, tag]) => [locale, {
    catalog: catalogs[locale],
    tag,
  }]),
) as Record<Locale, { catalog: string | undefined; tag: string }>;

const bundles = new Map<Locale, FluentBundle>();
const localizations = new Map<Locale, ReactLocalization>();

function getBundle(locale: Locale): FluentBundle {
  let bundle = bundles.get(locale);
  if (!bundle) {
    bundle = new FluentBundle(locales[locale].tag, { useIsolating: false });
    const errors = bundle.addResource(new FluentResource(locales[locale].catalog ?? ""));
    if (errors.length) throw errors[0];
    bundles.set(locale, bundle);
  }
  return bundle;
}

function getLocalization(locale: Locale): ReactLocalization {
  let localization = localizations.get(locale);
  if (!localization) {
    const chain = locale === DEFAULT_LOCALE
      ? [getBundle(DEFAULT_LOCALE)]
      : [getBundle(locale), getBundle(DEFAULT_LOCALE)];
    localization = new ReactLocalization(chain, null);
    localizations.set(locale, localization);
  }
  return localization;
}

function normalizeTag(language: string): string {
  return language.trim().toLowerCase().replace(/_/g, "-");
}

const languageAliases = new Map<string, Locale>(
  (Object.keys(steamLanguages) as Locale[]).flatMap((locale) =>
    [locale, steamLanguages[locale]].map((alias): [string, Locale] => [
      normalizeTag(alias),
      locale,
    ]),
  ),
);

export function normalizeLanguage(language: string): Locale {
  let tag = normalizeTag(language);
  while (tag) {
    const locale = languageAliases.get(tag);
    if (locale != null) return locales[locale].catalog != null ? locale : DEFAULT_LOCALE;
    const separator = tag.lastIndexOf("-");
    if (separator < 0) break;
    tag = tag.slice(0, separator);
  }
  return DEFAULT_LOCALE;
}

export function translate(
  locale: Locale,
  key: string,
  params: MessageParams = {},
): string {
  const localization = getLocalization(locale);
  const args = Object.fromEntries(Object.entries(params).map(([name, value]) => [
    name,
    typeof value === "object" ? translate(locale, value.key, value.params) : value,
  ]));
  return localization.getString(key, args, key);
}

const relativeTimeFormatters = new Map<Locale, Intl.RelativeTimeFormat>();
const relativeTimeUnits: readonly [number, Intl.RelativeTimeFormatUnit][] = [
  [365 * 24 * 60 * 60, "year"],
  [30 * 24 * 60 * 60, "month"],
  [7 * 24 * 60 * 60, "week"],
  [24 * 60 * 60, "day"],
  [60 * 60, "hour"],
  [60, "minute"],
  [1, "second"],
];

export function formatRelativeTime(locale: Locale, timestamp: number, now = Date.now()): string {
  let formatter = relativeTimeFormatters.get(locale);
  if (!formatter) {
    formatter = new Intl.RelativeTimeFormat(locales[locale].tag, { numeric: "always" });
    relativeTimeFormatters.set(locale, formatter);
  }
  const seconds = (timestamp - now) / 1000;
  const [divisor, unit] = relativeTimeUnits.find(([size]) => Math.abs(seconds) >= size)
    ?? relativeTimeUnits[relativeTimeUnits.length - 1];
  return formatter.format(Math.round(seconds / divisor), unit);
}

export function resolveMessage(
  locale: Locale,
  message: DisplayMessage,
): string {
  return typeof message === "string"
    ? message
    : translate(locale, message.key, message.params);
}

export function message(key: TranslationKey, params?: MessageParams): LocalizedMessage {
  return { key, params };
}

export class LocalizedError extends Error {
  readonly localizedMessage: LocalizedMessage;

  constructor(key: TranslationKey, params?: MessageParams) {
    super(translate(DEFAULT_LOCALE, key, params));
    this.localizedMessage = message(key, params);
  }
}

export function errorMessage(error: unknown): DisplayMessage {
  return error instanceof LocalizedError
    ? error.localizedMessage
    : error instanceof Error
      ? error.message
      : String(error);
}
