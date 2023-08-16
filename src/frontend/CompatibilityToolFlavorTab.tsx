import {
    ConfirmModal,
    DialogBody,
    DialogButton,
    DialogControlsSection,
    DialogControlsSectionHeader,
    Focusable,
    Menu, MenuItem,
    ProgressBarWithInfo,
    showContextMenu, showModal
} from 'decky-frontend-lib';
import {FaEllipsisH} from 'react-icons/fa';
import {
    AppState,
    Flavor,
    QueueCompatibilityToolState, GitHubRelease,
    Request,
    RequestType, SteamCompatibilityTool
} from "../types";
import {error, log} from '../logger';

export default function FlavorTab({ getAppState, getFlavor, getSocket}: { getAppState: AppState; getFlavor: Flavor; getSocket: WebSocket }) {
    const handleInstall = (release: GitHubRelease) => {
        if (getSocket && getSocket.readyState === WebSocket.OPEN) {
            const response: Request = {
                type: RequestType.Install,
                install: {
                    flavor: getFlavor.flavor,
                    install: release,
                    /*id: release.id,
                    tag_name: release.tag_name,
                    url: release.url,*/
                },
            };
            getSocket.send(JSON.stringify(response));
        } else {
            error("WebSocket not alive...");
        }
    };

    const handleUninstall = (release: SteamCompatibilityTool) => {
        log("are we being called here?");
        if (getSocket && getSocket.readyState === WebSocket.OPEN) {
            const response: Request = {
                type: RequestType.Uninstall,
                uninstall: {
                    flavor: getFlavor.flavor,
                    uninstall: release,
                    /*internal_name: release.internal_name, //fixme: we should pass back a directory instead
                    path: release.path,*/
                },
            };
            getSocket.send(JSON.stringify(response));
        } else {
            error("WebSocket not alive...");
        }
    };

    const handleUninstallModal = (release: SteamCompatibilityTool) => showModal(
        <ConfirmModal strTitle={"Uninstallation of " + release.display_name} strDescription={"Are you sure want to remove this compatibility tool?"} strOKButtonText={"Uninstall"} strCancelButtonText={"Cancel"} onOK={() => {handleUninstall(release)}}/>)

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
                                                        <MenuItem onClick={() => {
                                                            handleUninstallModal(release);
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
                                    <span>{release.tag_name} {getAppState.queue.filter(install => install.install.url == release.url).length == 1 && ("(In Queue)")}</span>
                                    {isItemQueued && (
                                        <div style={{marginLeft: 'auto', paddingLeft: '10px', minWidth: '200px'}}>
                                            <ProgressBarWithInfo nProgress={getAppState.in_progress?.progress}
                                                                 indeterminate={getAppState.in_progress?.state == QueueCompatibilityToolState.Extracting}
                                                                 sOperationText={getAppState.in_progress?.state} bottomSeparator="none"/>
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
                                                        <MenuItem onSelected={() => {
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