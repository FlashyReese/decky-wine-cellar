import {
    DialogBody,
    DialogButton,
    DialogControlsSection,
    DialogControlsSectionHeader,
    Focusable,
    Menu, MenuItem,
    ProgressBarWithInfo,
    showContextMenu
} from 'decky-frontend-lib';
import {FaEllipsisH} from 'react-icons/fa';
import {
    AppState,
    CompatibilityToolFlavor, Flavor,
    GitHubRelease,
    QueueCompatibilityToolState,
    Request,
    RequestType, SteamCompatibilityTool
} from "../types";
import {error} from '../logger';

export default function FlavorTab({ getAppState, getFlavor, getSocket}: { getAppState: AppState; getFlavor: Flavor; getSocket: WebSocket }) {
    const handleInstall = (release: GitHubRelease) => {
        if (getSocket && getSocket.readyState === WebSocket.OPEN) {
            const response: Request = {
                type: RequestType.Install,
                install: {
                    flavor: CompatibilityToolFlavor.ProtonGE,
                    url: release.url,
                },
            };
            getSocket.send(JSON.stringify(response));
        } else {
            error("WebSocket not alive...");
        }
    };

    const handleUninstall = (release: SteamCompatibilityTool) => {
        if (getSocket && getSocket.readyState === WebSocket.OPEN) {
            const response: Request = {
                type: RequestType.Uninstall,
                uninstall: {
                    flavor: CompatibilityToolFlavor.ProtonGE,
                    name: release.internal_name, //fixme: we should pass back a directory instead
                },
            };
            getSocket.send(JSON.stringify(response));
        } else {
            error("WebSocket not alive...");
        }
    };
    return (
        <DialogBody>
            {getFlavor.installed.length != 0 && (
                <DialogControlsSection>
                    <DialogControlsSectionHeader>
                        Installed
                    </DialogControlsSectionHeader>
                    <ul style={{listStyleType: 'none'}}>
                        {getFlavor.installed.map((release: SteamCompatibilityTool) => {
                            const isQueued = getAppState.in_progress !== null;
                            return (
                                <li style={{
                                    display: 'flex',
                                    flexDirection: 'row',
                                    alignItems: 'center',
                                    paddingBottom: '10px'
                                }}>
                                    <span>{release.display_name} {release.requires_restart && "(Requires Restart)"}{release.used_by_games.length != 0 && "(Used By Games)"}</span>
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
                                                        {release.requires_restart && (
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
            {getFlavor.not_installed.length != 0 && (
                <DialogControlsSection>
                    <DialogControlsSectionHeader>
                        Not Installed
                    </DialogControlsSectionHeader>
                    <ul>
                        {getFlavor.not_installed.map((release) => {
                            const isQueued = getAppState.in_progress !== null;
                            const isItemQueued = isQueued && getAppState.in_progress?.name === release.tag_name;
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
                                            <ProgressBarWithInfo nProgress={getAppState.in_progress?.progress}
                                                                 indeterminate={getAppState.in_progress?.state == QueueCompatibilityToolState.Extracting}
                                                                 sOperationText={getAppState.in_progress?.state}/>
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