import {DialogButton, Focusable, Menu, MenuItem, showContextMenu} from 'decky-frontend-lib';
import {FaEllipsisH} from 'react-icons/fa';
import {useEffect, useState} from 'react';

import {GitHubRelease, InstalledTool} from "../../types";
import {getInstalledCompatibilityTools} from "../../python_hook";

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
    const [state, setState] = useState<string>("Loading...");

    //serverApi.callPluginMethod("get_installed_compatibility_tools", {}).then(r => console.log(r));

    const [installedTools, setInstalledTools] = useState<InstalledTool[] | null>(null);
    const [availableReleases, setAvailableReleases] = useState<GitHubRelease[] | null>(null);

    const [installedCompatibilityTools, setInstalledCompatibilityTools] = useState<GitHubRelease[]>([])
    const [notInstalledCompatibilityTools, setNotInstalledCompatibilityTools] = useState<GitHubRelease[]>([])

    useEffect(() => {
        (async () => {
            const installedTools = await getInstalledCompatibilityTools();
            if (installedTools.success) {
                console.log(installedTools);
                console.log(installedTools.result);
                setInstalledTools(installedTools.result);
            } else {
                console.error("Failed to retrieve installed tools!");
                setState("Failed to retrieve installed tools!");
            }

            const availableReleases = await getGitHubReleases({getUrl});
            setAvailableReleases(availableReleases);

            if (availableReleases != null) {
                const installedCompatibilityTools = availableReleases.filter((release) =>
                    installedTools.result.map((install: InstalledTool) => install.name).includes(release.tag_name)
                );
                const notInstalledCompatibilityTools = availableReleases.filter((release) =>
                    !installedTools.result.map((install: InstalledTool) => install.name).includes(release.tag_name)
                );
                console.log(installedCompatibilityTools);
                // You can use the setState function or other logic to update the component state here
                setInstalledCompatibilityTools(installedCompatibilityTools);
                setNotInstalledCompatibilityTools(notInstalledCompatibilityTools);
            }
        })();
    }, [])


    if (!availableReleases) {
        return (
            <div>
                <p>{state}</p>
            </div>
        );
    }

    return (
        <ul style={{listStyleType: 'none'}}>
            {installedCompatibilityTools.map((release: GitHubRelease) => {
                return (
                    <li style={{display: 'flex', flexDirection: 'row', alignItems: 'center', paddingBottom: '10px'}}>
              <span>
                  {release.tag_name} ({installedTools?.find((install) => install.name == release.tag_name)?.status == "installed" ? "Installed" : "Installing"})
              </span>
                        <Focusable
                            style={{marginLeft: 'auto', boxShadow: 'none', display: 'flex', justifyContent: 'right'}}>
                            <DialogButton
                                style={{height: '40px', width: '40px', padding: '10px 12px', minWidth: '40px'}}
                                onClick={(e: MouseEvent) =>
                                    showContextMenu(
                                        <Menu label="Runner Actions">
                                            <MenuItem onSelected={() => {
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
                        <MenuItem onSelected={() => {}}> Install </MenuItem>
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