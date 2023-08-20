export type GitHubRelease = {
    url: String;
    id: number;
    draft: boolean;
    prerelease: boolean;
    tag_name: String;
    assets: Asset[];
    created_at: String;
    published_at: String;
    tarball_url: String;
    body: String;
}

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
}

export type AppState = {
    available_flavors: Flavor[];
    installed_compatibility_tools: SteamCompatibilityTool[];
    in_progress?: QueueCompatibilityTool | null
    queue: Install[]
}

export type Flavor = {
    flavor: CompatibilityToolFlavor;
    installed: SteamCompatibilityTool[];
    not_installed: GitHubRelease[];
}

export type Request = {
    type: RequestType;
    notification?: string | null;
    app_state?: AppState | null;
    install?: Install | null;
    uninstall?: Uninstall | null;
};

export type Install = {
    flavor: CompatibilityToolFlavor;
    install: GitHubRelease;
}

export type Uninstall = {
    flavor: CompatibilityToolFlavor;
    uninstall: SteamCompatibilityTool;
}

export type SteamCompatibilityTool = {
    path: string;
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