export type GitHubRelease = {
    url: string,
    id: string;
    tag_name: string;
};

export type Response = {
    type: ResponseType;
    message?: string | null;
    name?: string | null;
    url?: string | null;
    installed?: SteamCompatibilityTool[] | null;
    in_progress?: QueueCompatibilityTool | null;
};

export type SteamCompatibilityTool = {
    name: string;
    internal_name: string;
    display_name: string;
    version?: string | null;
    path: string;
    requires_restart: boolean;
}

export type QueueCompatibilityTool = {
    name: string;
    url: string;
    state: QueueCompatibilityToolState;
    progress: number;
}

export enum QueueCompatibilityToolState {
    Extracting = "Extracting",
    Downloading = "Downloading",
    Waiting = "Waiting",
}

export enum ResponseType {
    Install = "Install",
    Uninstall = "Uninstall",
    RequestState = "RequestState",
    UpdateState = "UpdateState",
    Notification = "Notification"
}