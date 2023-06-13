export type GitHubRelease = {
    url: string,
    id: string;
    tag_name: string;
};

export type AppState = {
    available_flavors: Flavor[],
    installed_compatibility_tools: SteamCompatibilityTool[],
    in_progress?: QueueCompatibilityTool | null
}

export type Flavor = {
    flavor: CompatibilityToolFlavor,
    installed: SteamCompatibilityTool[],
    not_installed: GitHubRelease[],
}

export type Request = {
    type: RequestType;
    app_state?: AppState | null;
    install?: Install | null;
    uninstall?: Uninstall | null;
};

export type Install = {
    flavor: CompatibilityToolFlavor,
    url: string,
}

export type Uninstall = {
    flavor: CompatibilityToolFlavor,
    name: string,
}

export type SteamCompatibilityTool = {
    //name: string;
    directory_name: string;
    internal_name: string;
    display_name: string;
    used_by_games: string[];
    requires_restart: boolean;
}

export type QueueCompatibilityTool = {
    flavor: CompatibilityToolFlavor;
    name: string;
    url: string;
    state: QueueCompatibilityToolState;
    progress: number;
}

export enum CompatibilityToolFlavor {
    ProtonGE = "ProtonGE",
    SteamTinkerLaunch = "SteamTinkerLaunch",
    Luxtorpeda = "Luxtorpeda",
    Boxtron = "Boxtron"
}

export enum QueueCompatibilityToolState {
    Extracting = "Extracting",
    Downloading = "Downloading",
    Waiting = "Waiting",
}

export enum RequestType {
    Install = "Install",
    Uninstall = "Uninstall",
    RequestState = "RequestState",
    UpdateState = "UpdateState",
    Notification = "Notification",
    Reboot = "Reboot",
}