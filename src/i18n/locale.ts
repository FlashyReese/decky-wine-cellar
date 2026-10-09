import { DEFAULT_LOCALE, Locale, normalizeLanguage } from "./core";

let locale: Locale = DEFAULT_LOCALE;
const listeners = new Set<() => void>();

export function getLocale(): Locale {
  return locale;
}

export function subscribeLocale(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function setLanguage(language: string): void {
  const next = normalizeLanguage(language);
  if (next !== locale) {
    locale = next;
    listeners.forEach((listener) => listener());
  }
}

export async function startLocalization(): Promise<void> {
  const fallback = navigator.language;
  setLanguage(fallback);
  setLanguage(await SteamClient.Settings.GetCurrentLanguage().catch(() => fallback));
}
