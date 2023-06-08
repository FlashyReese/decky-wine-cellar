import {DialogButton, Focusable, Menu, MenuItem, ProgressBarItem, showContextMenu} from 'decky-frontend-lib';
import {FaEllipsisH} from 'react-icons/fa';
import {useEffect, useState} from 'react';
import {GitHubRelease, QueueCompatibilityTool, Response, ResponseType, SteamCompatibilityTool} from "../../types";
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
    const [queuedTask, setQueuedTask] = useState<QueueCompatibilityTool | null>(null);

    const [socket, setSocket] = useState<WebSocket>();

    useEffect(() => {
        const socket = new WebSocket("ws://localhost:8887");
        setSocket(socket);

        socket.onopen = async () => {
            log('WebSocket connection established.');
            const response: Response = {
                type: ResponseType.RequestState
            };
            socket.send(JSON.stringify(response));
        };

        socket.onmessage = async (event) => {
            const response: Response = JSON.parse(event.data);
            if (response.type == ResponseType.UpdateState) {
                const availableReleases = await getGitHubReleases({getUrl});
                setAvailableReleases(availableReleases);

                if (availableReleases != null && response.installed != null) {
                    const installedCompatibilityTools = availableReleases.filter((release) =>
                        response.installed?.map((install: SteamCompatibilityTool) => install.name).includes(release.tag_name)
                    );
                    const notInstalledCompatibilityTools = availableReleases.filter((release) =>
                        !response.installed?.map((install: SteamCompatibilityTool) => install.name).includes(release.tag_name)
                    );
                    setInstalledCompatibilityTools(installedCompatibilityTools);
                    setNotInstalledCompatibilityTools(notInstalledCompatibilityTools);

                    if (response.in_progress == null) {
                        setQueuedTask(null);
                    } else {
                        setQueuedTask(response.in_progress);
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
            const response: Response = {
                type: ResponseType.Install,
                url: release.url,
            };
            socket.send(JSON.stringify(response));
            log("Requesting install of: " + release.id);
        } else {
            error("WebSocket not alive...");
        }
    };

    const handleUninstall = (release: GitHubRelease) => {
        if (socket && socket.readyState === WebSocket.OPEN) {
            const response: Response = {
                type: ResponseType.Uninstall,
                name: release.tag_name,
            };
            socket.send(JSON.stringify(response));
            log("Requesting uninstall of: " + release.id);
        } else {
            error("WebSocket not alive...");
        }
    };

    return (
        <ul style={{listStyleType: 'none'}}>
            {installedCompatibilityTools.map((release: GitHubRelease) => {
                const isQueued = queuedTask !== null;
                return (
                    <li style={{display: 'flex', flexDirection: 'row', alignItems: 'center', paddingBottom: '10px'}}>
                        <span>{release.tag_name} (Installed)</span>
                        <Focusable
                            style={{marginLeft: 'auto', boxShadow: 'none', display: 'flex', justifyContent: 'right'}}>
                            <DialogButton
                                style={{height: '40px', width: '40px', padding: '10px 12px', minWidth: '40px'}}
                                onClick={(e: MouseEvent) =>
                                    showContextMenu(
                                        <Menu label="Runner Actions">
                                            <MenuItem disabled={isQueued} onClick={() => {
                                                handleUninstall(release);
                                            }}>Uninstall</MenuItem>
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
            {notInstalledCompatibilityTools.map((release) => {
                const isQueued = queuedTask !== null;
                const isItemQueued = isQueued && queuedTask.name === release.tag_name;
                return (
                    <li style={{display: 'flex', flexDirection: 'row', alignItems: 'center', paddingBottom: '10px'}}>
                        <span>{release.tag_name} {isItemQueued ? "(" + queuedTask?.state + ")" : ""}</span>
                        {isItemQueued && (
                            <div style={{marginLeft: 'auto', paddingLeft: '10px', minWidth: '200px'}}>
                                <ProgressBarItem nProgress={queuedTask?.progress}/>
                            </div>
                        )}
                        <Focusable
                            style={{marginLeft: 'auto', boxShadow: 'none', display: 'flex', justifyContent: 'right'}}>
                            <DialogButton
                                style={{height: '40px', width: '40px', padding: '10px 12px', minWidth: '40px'}}
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
    );
}