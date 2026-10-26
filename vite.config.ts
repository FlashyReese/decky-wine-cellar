import { readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { defineConfig, type Plugin } from "vite";
import pluginExternal from "vite-plugin-external";

const manifest = JSON.parse(
  readFileSync(resolve(import.meta.dirname, "plugin.json"), "utf8"),
);
const localesDirectory = resolve(import.meta.dirname, "src/i18n/locales");
const catalogsModule = "\0virtual:fluent-catalogs";

const fluentCatalogs: Plugin = {
  name: "fluent-catalogs",
  resolveId(id) {
    return id === "virtual:fluent-catalogs" ? catalogsModule : null;
  },
  load(id) {
    if (id !== catalogsModule) return null;
    this.addWatchFile(localesDirectory);
    const catalogs: Record<string, string> = {};
    const files = readdirSync(localesDirectory, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.endsWith(".ftl"))
      .map((entry) => entry.name)
      .sort();
    for (const file of files) {
      const path = join(localesDirectory, file);
      this.addWatchFile(path);
      catalogs[file.slice(0, -4)] = readFileSync(path, "utf8");
    }
    if (catalogs.english == null)
      this.error("The English fallback catalog is required.");
    return `export default ${JSON.stringify(catalogs)};`;
  },
};

export default defineConfig({
  plugins: [
    fluentCatalogs,
    pluginExternal({
      externals: {
        react: "SP_REACT",
        "react-dom": "SP_REACTDOM",
        "react/jsx-runtime": "SP_JSX",
        "@decky/ui": "DFL",
        "@decky/manifest": JSON.stringify(manifest),
      },
    }),
  ],
  define: {
    "process.env.NODE_ENV": JSON.stringify(
      process.env.NODE_ENV || "production",
    ),
  },
  build: {
    target: "es2020",
    lib: {
      entry: resolve(import.meta.dirname, "src/index.tsx"),
      name: "WineCellar",
      formats: ["es"],
      fileName: "index",
    },
    sourcemap: true,
    rolldownOptions: {
      context: "window",
      external: ["react", "react-dom", "@decky/ui"],
      output: {
        format: "esm",
        sourcemapPathTransform: (relativeSourcePath) =>
          relativeSourcePath.replace(
            /^\.\.\//,
            `decky://decky/plugin/${encodeURIComponent(manifest.name)}/`,
          ),
        exports: "default",
      },
    },
  },
});
