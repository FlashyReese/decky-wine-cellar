import { useCallback, useSyncExternalStore } from "react";
import {
  DisplayMessage,
  MessageParams,
  resolveMessage,
  TranslationKey,
  translate,
} from "./core";
import { getLocale, subscribeLocale } from "./locale";

export * from "./core";
export { getLocale, startLocalization } from "./locale";

export function useTranslation() {
  const locale = useSyncExternalStore(subscribeLocale, getLocale, getLocale);
  const t = useCallback(
    (key: TranslationKey, params?: MessageParams) => translate(locale, key, params),
    [locale],
  );
  const translateMessage = useCallback(
    (message: DisplayMessage) => resolveMessage(locale, message),
    [locale],
  );
  return { t, locale, translateMessage };
}

export function translateMessage(message: DisplayMessage): string {
  return resolveMessage(getLocale(), message);
}
