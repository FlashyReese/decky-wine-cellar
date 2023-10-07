import { AppState } from "../types";
import { AppDetails, SteamAppOverview } from "decky-frontend-lib";

/**
 * Represents information about a compatibility tool.
 */
export interface CompatToolInfo {
  /** Name of the compatibility tool. */
  strToolName: string;
  /** Display name of the compatibility tool. */
  strDisplayName: string;
}

/**
 * Represents information about an installed application.
 */
export interface AppInfo {
  /** ID of the application. */
  nAppID: number;
  /** Name of the application. */
  strAppName: string;
  /** Sorting information for the application. */
  strSortAs: string;
  /** Last played time in Unix Epoch time format. */
  rtLastPlayed: number;
  /** Size of used storage by the application. */
  strUsedSize: string;
  /** Size of DLC storage used by the application. */
  strDLCSize: string;
  /** Size of workshop storage used by the application. */
  strWorkshopSize: string;
  /** Size of staged storage used by the application. */
  strStagedSize: string;
}

/**
 * Represents information about an installation folder.
 */
export interface InstallFolder {
  /** Index of the folder. */
  nFolderIndex: number;
  /** Path of the folder. */
  strFolderPath: string;
  /** User label for the folder. */
  strUserLabel: string;
  /** Name of the drive where the folder is located. */
  strDriveName: string;
  /** Total capacity of the folder. */
  strCapacity: string;
  /** Available free space in the folder. */
  strFreeSpace: string;
  /** Used space in the folder. */
  strUsedSize: string;
  /** Size of DLC storage used in the folder. */
  strDLCSize: string;
  /** Size of workshop storage used in the folder. */
  strWorkshopSize: string;
  /** Size of staged storage used in the folder. */
  strStagedSize: string;
  /** Indicates if the folder is set as the default installation folder. */
  bIsDefaultFolder: boolean;
  /** Indicates if the folder is currently mounted. */
  bIsMounted: boolean;
  /** Indicates if the folder is on a fixed drive. */
  bIsFixed: boolean;
  /** List of applications installed in the folder. */
  vecApps: AppInfo[];
}

/**
 * Retrieves a list of available compatibility tools for all applications.
 * @returns A Promise that resolves to an array of CompatToolInfo objects.
 */
export async function GetGlobalCompatTools(): Promise<CompatToolInfo[]> {
  try {
    const response = await SteamClient.Settings.GetGlobalCompatTools();
    // Map the response to CompatToolInfo objects and return as an array
    return response.map((tool: CompatToolInfo) => ({
      ...tool,
    })) as CompatToolInfo[];
  } catch (error) {
    // If an error occurs during the API call, log the error and return an empty array
    console.error("Error:", error);
    return [];
  }
}

/**
 * Retrieves a list of install folders.
 * @returns A Promise that resolves to an array of InstallFolder objects.
 */
export async function GetInstallFolders(): Promise<InstallFolder[]> {
  try {
    // Call SteamClient's method to get install folders
    const response = await SteamClient.InstallFolder.GetInstallFolders();
    // Map the response to InstallFolder objects and return as an array
    return response.map((tool: InstallFolder) => ({
      ...tool,
    })) as InstallFolder[];
  } catch (error) {
    // If an error occurs during the API call, log the error and return an empty array
    console.error("Error:", error);
    return [];
  }
}

/**
 * Clears the specified compatibility tool for a given application.
 * @param appId The ID of the application to clear compatibility tool for.
 */
export function ClearCompatTool(appId: number): void {
  SpecifyCompatTool(appId, "");
}

/**
 * Specifies a compatibility tool with the provided information for a given application.
 * @param appId The ID of the application to specify compatibility tool for.
 * @param toolName The CompatToolInfo object representing the compatibility tool to specify.
 */
export function SpecifyCompatToolWithInfo(
  appId: number,
  toolName: CompatToolInfo,
): void {
  SpecifyCompatTool(appId, toolName.strToolName);
}

/**
 * Specifies a compatibility tool by its name for a given application.
 * @param appId The ID of the application to specify compatibility tool for.
 * @param toolName The name of the compatibility tool to specify.
 */
export function SpecifyCompatTool(appId: number, toolName: string): void {
  SteamClient.Apps.SpecifyCompatTool(appId, toolName);
}

/**
 * Register a function to be executed when a shutdown start is detected.
 * @param action The function to be executed on shutdown start.
 */
export function RegisterForShutdownStart(action: () => void): any {
  return SteamClient.User.RegisterForShutdownStart(() => {
    action();
  });
}

/**
 * Restarts the Steam client.
 */
export function RestartSteamClient(): void {
  SteamClient.User.StartRestart();
}

export function GetInstalledApplications(appState: AppState): SteamApp[] {
  let installedApps: SteamApp[] = [];
  for (const steamAppCompat of appState.installed_applications) {
    let app: SteamAppOverview | null = window.appStore.GetAppOverviewByAppID(steamAppCompat.app_id);
    if (app && app.app_type == 1 /*Game*/) {// 2 - Application, 4 - Tool, 1073741824 - Shortcut
      let icon = window.appStore.GetIconURLForApp(app);
      const steamApp: SteamApp = {
        appId: steamAppCompat.app_id,
        name: steamAppCompat.display_name,
        icon: icon,
        specified_tool: steamAppCompat.strToolName || "",
      };
      installedApps.push(steamApp);
    }
  }
  return installedApps;
}

export interface SteamApp {
  appId: number;
  name: string;
  icon: string;
  specified_tool: string;
}

export interface AppData {
  details: AppDetails;
}
