import deckyPlugin from "@decky/rollup";
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const config = deckyPlugin();
const localesDirectory = fileURLToPath(new URL("./src/i18n/locales/", import.meta.url));
const catalogsModule = "\0virtual:fluent-catalogs";

config.plugins.unshift({
  name: "fluent-catalogs",
  resolveId(id) {
    return id === "virtual:fluent-catalogs" ? catalogsModule : null;
  },
  load(id) {
    if (id !== catalogsModule) return null;
    this.addWatchFile(localesDirectory);
    const catalogs = {};
    const files = readdirSync(localesDirectory, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(".ftl"))
      .map((entry) => entry.name)
      .sort();
    for (const file of files) {
      const path = join(localesDirectory, file);
      this.addWatchFile(path);
      catalogs[file.slice(0, -4)] = readFileSync(path, "utf8");
    }
    if (catalogs.english == null) this.error("The English fallback catalog is required.");
    return `export default ${JSON.stringify(catalogs)};`;
  },
});

export default config;
