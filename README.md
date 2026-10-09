# Wine Cellar
[![Project License](https://img.shields.io/github/license/FlashyReese/decky-wine-cellar?style=flat-square)](LICENSE)
[![Support Server](https://img.shields.io/discord/748363294749753484.svg?color=7289da&label=FlashyReese&logo=discord&style=flat-square)](https://discord.gg/MPHVG6MH4e)

A Decky Loader plugin to manage Steam Play compatibility tools.

## Contributing to localization

**By submitting translations, you are agreeing to have them licensed under the [Creative Commons Zero (CC0)](https://creativecommons.org/publicdomain/zero/1.0/) license.**

You can contribute to the translation effort by opening a [pull request](https://github.com/FlashyReese/decky-wine-cellar/pulls)! Start by [forking this repository](https://github.com/FlashyReese/decky-wine-cellar/fork) and editing your language's `.ftl` file in [`src/i18n/locales`](src/i18n/locales).

Translators can **translate** (add text for untranslated messages) or **suggest** (improve an existing translation). Keep message IDs and variable names such as `{ $tool }` unchanged, and save the file as UTF-8. See the [Fluent syntax guide](https://projectfluent.org/fluent/guide/) for help with variables, plurals, and other features.

### FAQ

#### A translation contains offensive wording. How do I report it?

We sincerely apologize. Community translations can occasionally include inappropriate wording. Please [open an issue](https://github.com/FlashyReese/decky-wine-cellar/issues) identifying the language and text so we can remove it.

#### How do I add a new language?

Find your language's **API language code** in [Steam's supported languages table](https://partner.steamgames.com/doc/store/localization/languages). Copy [`english.ftl`](src/i18n/locales/english.ftl) to `src/i18n/locales/<identifier>.ftl`, translate it, and open a pull request. For example, Latin American Spanish uses `latam.ftl`.

## License
Wine Cellar's code is licensed under [MIT](LICENSE). Translation catalogs in [`src/i18n/locales`](src/i18n/locales) use [CC0 1.0 Universal](https://creativecommons.org/publicdomain/zero/1.0/).
