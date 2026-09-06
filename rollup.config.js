import deckyPlugin from "@decky/rollup";

const config = deckyPlugin();

// Source maps are useful during development, but Decky CLI packages every file
// in dist. Disable them for the distributable plugin to avoid embedding sources.
config.output.sourcemap = false;

export default config;
