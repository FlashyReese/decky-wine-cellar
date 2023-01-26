import { ServerAPI } from "decky-frontend-lib";
import {GitHubRelease, InstalledTool} from "./types";

let server: ServerAPI | undefined = undefined;

export function setServer(s: ServerAPI) {
    server = s;
}

export async function getInstalledCompatibilityTools(): Promise<InstalledTool[]> {
    const response = await server!.callPluginMethod("get_installed_compatibility_tools", {});
    if (response.success) {
        const object = Object.create(response.result);
        return object.map((install: InstalledTool) => install);
    }
    return [];
}

export async function installAndExtract(
    release: GitHubRelease
): Promise<any> {
    return server!.callPluginMethod("install_and_extract", {"release": release})
}

export async function addToQueue(release: GitHubRelease): Promise<any> {
    return server!.callPluginMethod("add_to_queue", {"release": release})
}

export async function getReleaseInstallationProgress(
    release: GitHubRelease
): Promise<any> {
    return server!.callPluginMethod("get_release_installation_progress", {"release": release})
}
