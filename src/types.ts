import { CompatToolInfo } from "./utils/steamUtils";

export type GitHubRelease = {
  url: String;
  id: number;
  draft: boolean;
  prerelease: boolean;
  name: String;
  tag_name: String;
  assets: Asset[];
  created_at: String;
  published_at: String;
  tarball_url: String;
  body: String;
};

export type Asset = {
  url: String;
  id: number;
  name: String;
  content_type: String;
  state: String;
  size: number;
  download_count: number;
  created_at: String;
  updated_at: String;
};

export type AppState = {
  available_flavors: Flavor[];
  installed_compatibility_tools: SteamCompatibilityTool[];
  installed_applications: number[];
  in_progress?: QueueCompatibilityTool;
  task_queue: Task[];
  updater_state: UpdaterState;
  updater_last_check?: number;
};

export type Task = {
  type: TaskType;
  install?: Install;
  uninstall?: Uninstall;
};

export enum TaskType {
  CheckForFlavorUpdates = "CheckForFlavorUpdates",
  InstallCompatibilityTool = "InstallCompatibilityTool",
  CancelCompatibilityToolInstall = "CancelCompatibilityToolInstall",
  UninstallCompatibilityTool = "UninstallCompatibilityTool",
}

export type Flavor = {
  flavor: CompatibilityToolFlavor;
  releases: GitHubRelease[];
};

export type Request = {
  type: RequestType;
  task?: Task;
  available_compat_tools?: CompatToolInfo[];
  notification?: string;
  app_state?: AppState;
};

export type Install = {
  flavor: CompatibilityToolFlavor;
  release: GitHubRelease;
};

export type Uninstall = {
  flavor: CompatibilityToolFlavor;
  steam_compatibility_tool: SteamCompatibilityTool;
};

export type SteamCompatibilityTool = {
  path: string;
  //name: string;
  directory_name: string;
  internal_name: string;
  display_name: string;
  used_by_games: string[];
  requires_restart: boolean;
  flavor: CompatibilityToolFlavor;
  github_release?: GitHubRelease;
};

export type QueueCompatibilityTool = {
  flavor: CompatibilityToolFlavor;
  name: string;
  url: string;
  state: QueueCompatibilityToolState;
  progress: number;
};

export enum UpdaterState {
  Idle = "Idle",
  Checking = "Checking",
}

export enum CompatibilityToolFlavor {
  Unknown = "Unknown",
  ProtonGE = "ProtonGE",
  //SteamTinkerLaunch = "SteamTinkerLaunch",
  Luxtorpeda = "Luxtorpeda",
  Boxtron = "Boxtron",
}

export enum QueueCompatibilityToolState {
  Extracting = "Extracting",
  Downloading = "Downloading",
  Waiting = "Waiting",
}

export enum RequestType {
  Task = "Task",
  RequestState = "RequestState",
  UpdateState = "UpdateState",
  Notification = "Notification",
}
