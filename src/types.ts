import { CompatToolInfo } from "./utils/steamUtils";

export type GitHubRelease = {
  url: string;
  id: number;
  draft: boolean;
  prerelease: boolean;
  name: string;
  tag_name: string;
  assets: Asset[];
  created_at: string;
  published_at: string;
  tarball_url: string;
  body: string;
};

export type Asset = {
  url: string;
  id: number;
  name: string;
  content_type: string;
  state: string;
  size: number;
  download_count: number;
  created_at: string;
  updated_at: string;
  browser_download_url: string;
};

export type CatalogRelease = {
  id: string;
  flavor: CompatibilityToolFlavor;
  release: GitHubRelease;
};

export type Flavor = {
  flavor: CompatibilityToolFlavor;
  releases: CatalogRelease[];
};

export type InstalledCompatibilityTool = {
  id: string;
  path: string;
  directory_name: string;
  display_name: string;
  internal_name: string;
  used_by_games: string[];
  requires_restart: boolean;
  flavor: CompatibilityToolFlavor;
  catalog_release_id?: string;
  github_release?: GitHubRelease;
};

export enum OperationKind {
  Install = "Install",
  Uninstall = "Uninstall",
}

export enum OperationState {
  Pending = "Pending",
  Running = "Running",
  Downloading = "Downloading",
  Extracting = "Extracting",
  Cancelling = "Cancelling",
}

export type DownloadProgress = {
  bytes_downloaded: number;
  total_bytes: number | null;
  bytes_per_second: number | null;
  eta_seconds: number | null;
  elapsed_seconds: number;
};

export type OperationInfo = {
  id: string;
  label: string;
  kind: OperationKind;
  state: OperationState;
  progress: number;
  release_id?: string;
  installed_tool_id?: string;
  download?: DownloadProgress | null;
};

export enum UpdaterState {
  Idle = "Idle",
  Checking = "Checking",
}

export type AppState = {
  catalog_flavors: Flavor[];
  installed_tools: InstalledCompatibilityTool[];
  app_compat_tool_mappings: Record<string, string>;
  app_compat_tool_mappings_stale: boolean;
  steam_visible_tools: CompatToolInfo[];
  current_operation?: OperationInfo;
  queued_operations: OperationInfo[];
  updater_state: UpdaterState;
  updater_last_check?: number;
};

export enum CommandType {
  RefreshCatalog = "RefreshCatalog",
  InstallCatalogRelease = "InstallCatalogRelease",
  UninstallInstalledTool = "UninstallInstalledTool",
  CancelOperation = "CancelOperation",
}

export type Command =
  | {
      type: CommandType.RefreshCatalog;
    }
  | {
      type: CommandType.InstallCatalogRelease;
      release_id: string;
    }
  | {
      type: CommandType.UninstallInstalledTool;
      installed_tool_id: string;
    }
  | {
      type: CommandType.CancelOperation;
      operation_id: string;
    };

export enum MessageType {
  GetState = "GetState",
  ReportSteamVisibleTools = "ReportSteamVisibleTools",
  Command = "Command",
  UpdateState = "UpdateState",
  UpdateOperations = "UpdateOperations",
  Notification = "Notification",
}

export type OperationStateSnapshot = {
  current_operation?: OperationInfo;
  queued_operations: OperationInfo[];
};

export type MessageEnvelope = {
  type: MessageType;
  command?: Command;
  steam_visible_tools?: CompatToolInfo[];
  notification?: string;
  app_state?: AppState;
  operation_state?: OperationStateSnapshot;
};

export enum CompatibilityToolFlavor {
  Unknown = "Unknown",
  ProtonGE = "ProtonGE",
  ProtonCachyOS = "ProtonCachyOS",
  SteamTinkerLaunch = "SteamTinkerLaunch",
  Luxtorpeda = "Luxtorpeda",
  Boxtron = "Boxtron",
}
