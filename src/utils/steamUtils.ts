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
 * Retrieves a list of available compatibility tools for a specific application.
 * @param appId The ID of the application to retrieve compatibility tools for.
 * @returns A Promise that resolves to an array of CompatToolInfo objects.
 */
export async function GetAvailableCompatTools(
  appId: number,
): Promise<CompatToolInfo[]> {
  try {
    // Call SteamClient's method to get available compatibility tools for the specified app
    const response = await SteamClient.Apps.GetAvailableCompatTools(appId);
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