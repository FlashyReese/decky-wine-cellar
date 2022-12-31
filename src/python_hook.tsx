import { ServerAPI } from "decky-frontend-lib";

let server: ServerAPI | undefined = undefined;

export function setServer(s: ServerAPI) {
    server = s;
}

export async function getInstalledCompatibilityTools(): Promise<any> {
    return server!.callPluginMethod("get_installed_compatibility_tools", {});
}
