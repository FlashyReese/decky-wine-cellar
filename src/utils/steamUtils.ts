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

export type ManagedSteamApplication = {
  appId: number;
  name: string;
  sortAs: string;
  iconUrl?: string;
  isShortcut: boolean;
};

export type ManagedApplicationInventory = {
  applications: ManagedSteamApplication[];
  warning?: string;
};

type ManagedAppOverview = {
  appid: number;
  app_type: number;
  display_name?: string;
  sort_as?: string;
  BIsShortcut?: () => boolean;
};

type ManagedAppStore = {
  allApps?: readonly ManagedAppOverview[] | Iterable<ManagedAppOverview>;
  GetAppOverviewByAppID?: (
    appId: number,
  ) => ManagedAppOverview | null | undefined;
  GetIconURLForApp?: (app: ManagedAppOverview) => string | null | undefined;
};

type InstalledAppMetadata = {
  name: string;
  sortAs: string;
};

const SHORTCUT_APP_TYPE = 1_073_741_824;
const COMPAT_TOOLS_TIMEOUT_MS = 5_000;

function getAppOverviews(appStore: ManagedAppStore): {
  overviews: ManagedAppOverview[];
  isComplete: boolean;
} {
  try {
    return appStore.allApps == null
      ? { overviews: [], isComplete: false }
      : { overviews: Array.from(appStore.allApps), isComplete: true };
  } catch (error) {
    console.error("Unable to read Steam application inventory:", error);
    return { overviews: [], isComplete: false };
  }
}

function getAppOverview(
  appStore: ManagedAppStore,
  appId: number,
): ManagedAppOverview | undefined {
  try {
    return appStore.GetAppOverviewByAppID?.call(appStore, appId) ?? undefined;
  } catch (error) {
    console.error(`Unable to read Steam application ${appId}:`, error);
    return undefined;
  }
}

function isShortcut(overview: ManagedAppOverview): boolean {
  const appTypeFallback = overview.app_type === SHORTCUT_APP_TYPE;

  try {
    return overview.BIsShortcut?.() ?? appTypeFallback;
  } catch (error) {
    console.error(`Unable to identify Steam shortcut ${overview.appid}:`, error);
    return appTypeFallback;
  }
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
 * Retrieves the compatibility tools Steam allows for a specific application.
 * Unlike the global helper, failures are allowed to propagate so callers can
 * distinguish an unavailable menu from an application with no compatible tools.
 */
export async function GetAvailableCompatTools(
  appId: number,
): Promise<CompatToolInfo[]> {
  let timeout: ReturnType<typeof setTimeout> | undefined;

  try {
    const response = await Promise.race([
      SteamClient.Apps.GetAvailableCompatTools(appId),
      new Promise<never>((_resolve, reject) => {
        timeout = setTimeout(
          () => reject(new Error("Steam did not return compatible tools in time.")),
          COMPAT_TOOLS_TIMEOUT_MS,
        );
      }),
    ]);

    return response.map((tool) => ({
      strToolName: tool.strToolName,
      strDisplayName: tool.strDisplayName,
    }));
  } finally {
    if (timeout != null) {
      clearTimeout(timeout);
    }
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
 * Retrieves installed Steam games and library shortcuts without changing their
 * compatibility settings.
 * @returns Applications plus a warning when Steam's shortcut inventory is not ready.
 */
export async function GetManagedApplications(): Promise<
  ManagedApplicationInventory
> {
  const installedApps = new Map<number, InstalledAppMetadata>();
  // This intentionally bypasses the forgiving public GetInstallFolders helper:
  // callers of this page-specific inventory need to surface an RPC failure
  // instead of presenting it as an empty library.
  const installFolders = await SteamClient.InstallFolder.GetInstallFolders();

  for (const folder of installFolders) {
    for (const app of folder.vecApps) {
      if (!Number.isSafeInteger(app.nAppID) || app.nAppID <= 0) {
        continue;
      }

      installedApps.set(app.nAppID, {
        name: app.strAppName,
        sortAs: app.strSortAs,
      });
    }
  }

  const appStore = window.appStore as unknown as ManagedAppStore | undefined;
  let isPartialInventory = appStore == null;

  const overviewsById = new Map<number, ManagedAppOverview>();
  if (appStore != null) {
    const overviewInventory = getAppOverviews(appStore);
    isPartialInventory = !overviewInventory.isComplete;
    for (const overview of overviewInventory.overviews) {
      if (Number.isSafeInteger(overview.appid) && overview.appid > 0) {
        overviewsById.set(overview.appid, overview);
      }
    }

    // allApps is the only reliable source for every non-Steam shortcut. Fill in
    // missing installed games individually when that collection is unavailable
    // or has not finished populating yet.
    for (const appId of installedApps.keys()) {
      if (!overviewsById.has(appId)) {
        const overview = getAppOverview(appStore, appId);
        if (overview != null) {
          overviewsById.set(appId, overview);
        }
      }
    }
  }

  const applications = new Map<
    number,
    { overview?: ManagedAppOverview; isShortcut: boolean }
  >();
  for (const overview of overviewsById.values()) {
    const shortcut = isShortcut(overview);
    const isInstalledGame =
      overview.app_type === 1 && installedApps.has(overview.appid);

    if (shortcut || isInstalledGame) {
      applications.set(overview.appid, {
        overview,
        isShortcut: shortcut,
      });
    }
  }

  // Steam's overview store is populated lazily during startup. Retain an
  // installed-folder fallback so a temporarily missing overview does not make
  // an installed game disappear from the page; a later refresh enriches it.
  for (const appId of installedApps.keys()) {
    if (!overviewsById.has(appId)) {
      applications.set(appId, { isShortcut: false });
    }
  }

  const managedApplications = Array.from(applications.entries()).map(
    ([appId, { overview, isShortcut: shortcut }]) => {
      const installedMetadata = installedApps.get(appId);
      const name =
        overview?.display_name || installedMetadata?.name || String(appId);
      const sortAs = overview?.sort_as || installedMetadata?.sortAs || name;

      let iconUrl: string | undefined;
      if (appStore != null && overview != null) {
        try {
          iconUrl =
            appStore.GetIconURLForApp?.call(appStore, overview) || undefined;
        } catch (error) {
          console.error(
            `Unable to read the icon for Steam application ${appId}:`,
            error,
          );
        }
      }

      return {
        appId,
        name,
        sortAs,
        iconUrl,
        isShortcut: shortcut,
      };
    },
  );

  const collator = new Intl.Collator(undefined, {
    numeric: true,
    sensitivity: "base",
  });
  const sortedApplications = managedApplications.sort(
    (left, right) =>
      collator.compare(left.sortAs || left.name, right.sortAs || right.name) ||
      left.appId - right.appId,
  );

  return {
    applications: sortedApplications,
    warning: isPartialInventory
      ? "Steam's shortcut catalog is still loading. Installed Steam titles are shown; refresh to include non-Steam shortcuts."
      : undefined,
  };
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
  SteamClient.User.StartRestart(false);
}
