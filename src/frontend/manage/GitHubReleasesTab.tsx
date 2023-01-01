import {DialogButton, Focusable, Menu, MenuItem, showContextMenu} from 'decky-frontend-lib';
import {FaEllipsisH} from 'react-icons/fa';
import {useEffect, useState} from 'react';

import {GitHubRelease, InstalledTool} from "../../types";
import {getInstalledCompatibilityTools, getReleaseInstallationProgress, installAndExtract} from "../../python_hook";

export async function getGitHubReleases({getUrl}: { getUrl: string; }): Promise<GitHubRelease[]> {
    return fetch(getUrl, {
        method: 'GET',
    }).then(r => r.json());
}

export default function GitHubReleasesList({
                                               getUrl
                                           }: {
    getUrl: string;
}) {
    //onst [installedTools, setInstalledTools] = useState<InstalledTool[]>([]);
    const [availableReleases, setAvailableReleases] = useState<GitHubRelease[] | null>(null);

    const [installedCompatibilityTools, setInstalledCompatibilityTools] = useState<GitHubRelease[]>([])
    const [notInstalledCompatibilityTools, setNotInstalledCompatibilityTools] = useState<GitHubRelease[]>([])
    const [progressMap, setProgressMap] = useState(new Map);

    useEffect(() => {
        (async () => {
            const installedTools = await getInstalledCompatibilityTools();
            // setInstalledTools(installedTools);

            const availableReleases = await getGitHubReleases({getUrl});
            setAvailableReleases(availableReleases);

            if (availableReleases != null) {
                const installedCompatibilityTools = availableReleases.filter((release) =>
                    installedTools.map((install: InstalledTool) => install.name).includes(release.tag_name)
                );
                const notInstalledCompatibilityTools = availableReleases.filter((release) =>
                    !installedTools.map((install: InstalledTool) => install.name).includes(release.tag_name)
                );
                setInstalledCompatibilityTools(installedCompatibilityTools);
                setNotInstalledCompatibilityTools(notInstalledCompatibilityTools);

                const pendingInstall = installedCompatibilityTools.filter((release: GitHubRelease) => installedTools.find((install) => install.name == release.tag_name)?.status == "in_progress");
                const freshMap = new Map();
                pendingInstall.map(async (release: GitHubRelease) => {
                    const result = await getReleaseInstallationProgress(release);
                    console.log(release.id + ": " + result)
                    freshMap.set(release.id, result.result);
                })
                setProgressMap(freshMap);
            }
        })();
    }, [])


    if (!availableReleases) {
        return (
            <div>
                <p>Loading...</p>
            </div>
        );
    }

    return (
        <ul style={{listStyleType: 'none'}}>
            {installedCompatibilityTools.map((release: GitHubRelease) => {
                return (
                    <li style={{display: 'flex', flexDirection: 'row', alignItems: 'center', paddingBottom: '10px'}}>
              <span>
                  {release.tag_name} ({progressMap.has(release.id) ? "Installing... " + progressMap.get(release.id) + "%" : "Installed"})
              </span>
                        <Focusable
                            style={{marginLeft: 'auto', boxShadow: 'none', display: 'flex', justifyContent: 'right'}}>
                            <DialogButton
                                style={{height: '40px', width: '40px', padding: '10px 12px', minWidth: '40px'}}
                                onClick={(e: MouseEvent) =>
                                    showContextMenu(
                                        <Menu label="Runner Actions">
                                            <MenuItem onClick={() => {
                                                console.log("Requesting to uninstall: " + release.tag_name)
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
                return (
                    <li style={{ display: 'flex', flexDirection: 'row', alignItems: 'center', paddingBottom: '10px' }}>
              <span>
                  {release.tag_name}
              </span>
                        <Focusable style={{ marginLeft: 'auto', boxShadow: 'none', display: 'flex', justifyContent: 'right' }}>
                            <DialogButton
                                style={{ height: '40px', width: '40px', padding: '10px 12px', minWidth: '40px' }}
                                onClick={(e: MouseEvent) =>
                                    showContextMenu(
                                        <Menu label="Runner Actions">
                                            <MenuItem onSelected={() => {}} onClick={() => {
                                                console.log("Requesting to install: " + release.tag_name);
                                                (async () => {
                                                    const install = await installAndExtract(release);
                                                    if (install.success) {
                                                        console.log(install.result);
                                                    }
                                                    console.log(install);
                                                })();
                                            }}> Install </MenuItem>
                                        </Menu>,
                                        e.currentTarget ?? window,
                                    )
                                }
                            >
                                <FaEllipsisH />
                            </DialogButton>
                        </Focusable>
                    </li>
                );
            })}
        </ul>
    );
}