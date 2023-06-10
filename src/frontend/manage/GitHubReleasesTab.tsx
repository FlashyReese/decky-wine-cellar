import {
    DialogBody,
    DialogButton,
    DialogControlsSection,
    DialogControlsSectionHeader,
    Focusable,
    Menu,
    MenuItem,
    ProgressBarWithInfo,
    showContextMenu
} from 'decky-frontend-lib';
import {FaEllipsisH} from 'react-icons/fa';
import {useEffect, useState} from 'react';
import {
    AppState,
    CompatibilityToolFlavor,
    GitHubRelease,
    QueueCompatibilityToolState,
    Request,
    RequestType,
    SteamCompatibilityTool
} from "../../types";
import {error, log} from '../../logger';

export async function getGitHubReleases({getUrl}: { getUrl: string; }): Promise<GitHubRelease[]> {
    return fetch(getUrl, {
        method: 'GET',
    }).then(r => r.json());
}

export default function GitHubReleasesList({getUrl}: { getUrl: string; }) {
    const [availableReleases, setAvailableReleases] = useState<GitHubRelease[] | null>(null);

    const [installedCompatibilityTools, setInstalledCompatibilityTools] = useState<GitHubRelease[]>([])
    const [notInstalledCompatibilityTools, setNotInstalledCompatibilityTools] = useState<GitHubRelease[]>([])
    const [appState, setAppState] = useState<AppState | null>(null);

    const [socket, setSocket] = useState<WebSocket>();

    useEffect(() => {
        const socket = new WebSocket("ws://localhost:8887");
        setSocket(socket);

        socket.onopen = async () => {
            log('WebSocket connection established.');
            const response: Request = {
                type: RequestType.RequestState
            };
            socket.send(JSON.stringify(response));
        };

        socket.onmessage = async (event) => {
            const response: Request = JSON.parse(event.data);
            if (response.type == RequestType.UpdateState) {
                const availableReleases = await getGitHubReleases({getUrl});
                setAvailableReleases(availableReleases);
                if (response.app_state != null) {
                    if (availableReleases != null) {
                        const installedCompatibilityTools = availableReleases.filter((release) =>
                            response.app_state?.installed_compatibility_tools.map((install: SteamCompatibilityTool) => install.name).includes(release.tag_name)
                        );
                        const notInstalledCompatibilityTools = availableReleases.filter((release) =>
                            !response.app_state?.installed_compatibility_tools?.map((install: SteamCompatibilityTool) => install.name).includes(release.tag_name)
                        );
                        setAppState(response.app_state);
                        setInstalledCompatibilityTools(installedCompatibilityTools);
                        setNotInstalledCompatibilityTools(notInstalledCompatibilityTools);
                    }
                }

            }
        };

        socket.onclose = () => {
            log('WebSocket connection closed.');
        };

        return () => {
            socket.close(); // Close the WebSocket connection on component unmount
        };
    }, [])


    if (!availableReleases) {
        return (
            <div>
                <p>Loading...</p>
            </div>
        );
    }

    const handleInstall = (release: GitHubRelease) => {
        if (socket && socket.readyState === WebSocket.OPEN) {
            const response: Request = {
                type: RequestType.Install,
                install: {
                    flavor: CompatibilityToolFlavor.ProtonGE,
                    url: release.url,
                },
            };
            socket.send(JSON.stringify(response));
            log("Requesting install of: " + release.id);
        } else {
            error("WebSocket not alive...");
        }
    };

    const handleUninstall = (release: GitHubRelease) => {
        if (socket && socket.readyState === WebSocket.OPEN) {
            const response: Request = {
                type: RequestType.Uninstall,
                uninstall: {
                    flavor: CompatibilityToolFlavor.ProtonGE,
                    name: release.tag_name,
                },
            };
            socket.send(JSON.stringify(response));
            log("Requesting uninstall of: " + release.id);
        } else {
            error("WebSocket not alive...");
        }
    };
    return (
        <DialogBody>
            {installedCompatibilityTools.length != 0 && (
                <DialogControlsSection>
                    <DialogControlsSectionHeader>
                        Installed
                    </DialogControlsSectionHeader>
                    <ul style={{listStyleType: 'none'}}>
                        {installedCompatibilityTools.map((release: GitHubRelease) => {
                            const isQueued = appState?.in_progress !== null;
                            const requiresRestart = appState?.installed_compatibility_tools.filter(ct => (ct.name == release.tag_name || ct.display_name == release.tag_name || ct.internal_name == release.tag_name) && ct.requires_restart).length == 1;
                            const usedByGame = appState?.installed_compatibility_tools.filter(ct => (ct.name == release.tag_name || ct.display_name == release.tag_name || ct.internal_name == release.tag_name) && ct.used_by_games.length != 0).length == 1;
                            return (
                                <li style={{
                                    display: 'flex',
                                    flexDirection: 'row',
                                    alignItems: 'center',
                                    paddingBottom: '10px'
                                }}>
                                    <span>{release.tag_name} {requiresRestart && "(Requires Restart)"}{usedByGame && "(Used By Games)"}</span>
                                    <Focusable
                                        style={{
                                            marginLeft: 'auto',
                                            boxShadow: 'none',
                                            display: 'flex',
                                            justifyContent: 'right'
                                        }}>
                                        <DialogButton
                                            style={{
                                                height: '40px',
                                                width: '40px',
                                                padding: '10px 12px',
                                                minWidth: '40px'
                                            }}
                                            onClick={(e: MouseEvent) =>
                                                showContextMenu(
                                                    <Menu label="Runner Actions">
                                                        <MenuItem disabled={isQueued} onClick={() => {
                                                            handleUninstall(release);
                                                        }}>Uninstall</MenuItem>
                                                        {requiresRestart && (
                                                            <MenuItem disabled={isQueued} onClick={() => {
                                                                SteamClient.User.StartRestart();
                                                            }}>Restart Steam</MenuItem>
                                                        )}
                                                    </Menu>,
                                                    e.currentTarget ?? window,
                                                )
                                            }
                                        >
                                            <FaEllipsisH/>
                                        </DialogButton>
                                    </Focusable>
                                </li>
                            );
                        })}
                    </ul>
                </DialogControlsSection>
            )}
            {notInstalledCompatibilityTools.length != 0 && (
                <DialogControlsSection>
                    <DialogControlsSectionHeader>
                        Not Installed
                    </DialogControlsSectionHeader>
                    <ul>
                        {notInstalledCompatibilityTools.map((release) => {
                            const isQueued = appState?.in_progress !== null;
                            const isItemQueued = isQueued && appState?.in_progress?.name === release.tag_name;
                            return (
                                <li style={{
                                    display: 'flex',
                                    flexDirection: 'row',
                                    alignItems: 'center',
                                    paddingBottom: '10px'
                                }}>
                                    <span>{release.tag_name}</span>
                                    {isItemQueued && (
                                        <div style={{marginLeft: 'auto', paddingLeft: '10px', minWidth: '200px'}}>
                                            <ProgressBarWithInfo nProgress={appState?.in_progress?.progress}
                                                                 indeterminate={appState?.in_progress?.state == QueueCompatibilityToolState.Extracting}
                                                                 sOperationText={appState?.in_progress?.state}/>
                                        </div>
                                    )}
                                    <Focusable
                                        style={{
                                            marginLeft: 'auto',
                                            boxShadow: 'none',
                                            display: 'flex',
                                            justifyContent: 'right'
                                        }}>
                                        <DialogButton
                                            style={{
                                                height: '40px',
                                                width: '40px',
                                                padding: '10px 12px',
                                                minWidth: '40px'
                                            }}
                                            onClick={(e: MouseEvent) =>
                                                showContextMenu(
                                                    <Menu label="Runner Actions">
                                                        <MenuItem disabled={isQueued} onSelected={() => {
                                                        }} onClick={() => {
                                                            handleInstall(release);
                                                        }}>Install</MenuItem>
                                                    </Menu>,
                                                    e.currentTarget ?? window,
                                                )
                                            }
                                        >
                                            <FaEllipsisH/>
                                        </DialogButton>
                                    </Focusable>
                                </li>
                            );
                        })}
                    </ul>
                </DialogControlsSection>
            )}
        </DialogBody>
    );
}