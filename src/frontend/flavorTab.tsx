import {
  ConfirmModal,
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Focusable,
  Menu,
  MenuItem,
  ProgressBarWithInfo,
  showContextMenu,
  showModal,
} from "@decky/ui";
import { FaEllipsisH } from "react-icons/fa";
import {
  AppState,
  Flavor,
  GitHubRelease,
  QueueCompatibilityToolState,
  Request,
  RequestType,
  SteamCompatibilityTool,
  TaskType,
} from "../types";
import { error } from "../utils/logger";
import ChangeLogModal from "../components/changeLogModal";
import { RestartSteamClient } from "../utils/steamUtils";

export default function FlavorTab({
  appState,
  flavor,
  socket,
}: {
  appState: AppState;
  flavor: Flavor;
  socket: WebSocket;
}) {
  const handleInstall = (gitHubRelease: GitHubRelease) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Task,
        task: {
          type: TaskType.InstallCompatibilityTool,
          install: {
            flavor: flavor.flavor,
            release: gitHubRelease,
          },
        },
      };
      socket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleUninstall = (steamCompatibilityTool: SteamCompatibilityTool) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Task,
        task: {
          type: TaskType.UninstallCompatibilityTool,
          uninstall: {
            flavor: flavor.flavor,
            steam_compatibility_tool: steamCompatibilityTool,
          },
        },
      };
      socket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleViewUsedByGames = (
    steamCompatibilityTool: SteamCompatibilityTool,
  ) => {
    showModal(
      <ConfirmModal
        strTitle={
          "Steam Applications using " + steamCompatibilityTool.display_name
        }
        strDescription={steamCompatibilityTool.used_by_games.join(", ")}
        strOKButtonText={"OK"}
      />,
    );
  };

  const handleCancel = (gitHubRelease: GitHubRelease) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Task,
        task: {
          type: TaskType.CancelCompatibilityToolInstall,
          install: {
            flavor: flavor.flavor,
            release: gitHubRelease,
          },
        },
      };
      socket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleUninstallModal = (
    steamCompatibilityTool: SteamCompatibilityTool,
  ) =>
    showModal(
      <ConfirmModal
        strTitle={"Uninstallation of " + steamCompatibilityTool.display_name}
        strDescription={
          "Are you sure want to remove this compatibility tool? Used by " +
          steamCompatibilityTool.used_by_games.join(",")
        }
        strOKButtonText={"Uninstall"}
        strCancelButtonText={"Cancel"}
        onOK={() => {
          handleUninstall(steamCompatibilityTool);
        }}
      />,
    );

  const handleViewChangeLog = (gitHubRelease: GitHubRelease) =>
    showModal(<ChangeLogModal release={gitHubRelease} />);

  return (
    <DialogBody>
      {appState.installed_compatibility_tools.filter(
        (t) => t.flavor == flavor.flavor,
      ).length != 0 && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
          <ul style={{ listStyleType: "none" }}>
            {appState.installed_compatibility_tools
              .filter((t) => t.flavor == flavor.flavor)
              .map((steamCompatibilityTool: SteamCompatibilityTool) => {
                const isQueued = appState.in_progress !== null;
                return (
                  <li
                    style={{
                      display: "flex",
                      flexDirection: "row",
                      alignItems: "center",
                      paddingBottom: "10px",
                    }}
                  >
                    <span>
                      {steamCompatibilityTool.display_name}{" "}
                      {steamCompatibilityTool.requires_restart &&
                        "(Requires Restart)"}
                      {steamCompatibilityTool.used_by_games.length != 0 &&
                        "(Used By Games)"}
                    </span>
                    <Focusable
                      style={{
                        marginLeft: "auto",
                        boxShadow: "none",
                        display: "flex",
                        justifyContent: "right",
                      }}
                    >
                      <DialogButton
                        style={{
                          height: "40px",
                          width: "40px",
                          padding: "10px 12px",
                          minWidth: "40px",
                        }}
                        onClick={(e: MouseEvent) =>
                          showContextMenu(
                            <Menu label="Runner Actions">
                              <MenuItem
                                onClick={() => {
                                  handleUninstallModal(steamCompatibilityTool);
                                }}
                              >
                                Uninstall
                              </MenuItem>
                              {steamCompatibilityTool.used_by_games.length !=
                                0 && (
                                <MenuItem
                                  onSelected={() => {}}
                                  onClick={() => {
                                    handleViewUsedByGames(
                                      steamCompatibilityTool,
                                    );
                                  }}
                                >
                                  View Used By Games
                                </MenuItem>
                              )}
                              {steamCompatibilityTool.github_release !=
                                null && (
                                <MenuItem
                                  onClick={() => {
                                    if (
                                      steamCompatibilityTool.github_release !=
                                      null
                                    ) {
                                      handleViewChangeLog(
                                        steamCompatibilityTool.github_release,
                                      );
                                    }
                                  }}
                                >
                                  View Change Log
                                </MenuItem>
                              )}
                              {steamCompatibilityTool.requires_restart && (
                                <MenuItem
                                  disabled={isQueued}
                                  onClick={() => {
                                    RestartSteamClient();
                                  }}
                                >
                                  Restart Steam
                                </MenuItem>
                              )}
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
        </DialogControlsSection>
      )}
      {flavor.releases.length != 0 && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>
            Not Installed
          </DialogControlsSectionHeader>
          <ul>
            {flavor.releases.map((release) => {
              const isQueued =
                appState.task_queue
                  .filter(
                    (task) => task.type == TaskType.InstallCompatibilityTool,
                  )
                  .map((task) => task.install)
                  .filter(
                    (install) =>
                      install != null && install.release.url == release.url,
                  ).length == 1;
              const isInProgress = appState.in_progress !== null;
              const isItemInProgress =
                isInProgress && appState.in_progress?.name === release.tag_name;
              return (
                <li
                  style={{
                    display: "flex",
                    flexDirection: "row",
                    alignItems: "center",
                    paddingBottom: "10px",
                  }}
                >
                  <span>
                    {release.tag_name}
                    {isQueued && " (In Queue)"}
                  </span>
                  {isItemInProgress && (
                    <div
                      style={{
                        marginLeft: "auto",
                        paddingLeft: "10px",
                        minWidth: "200px",
                      }}
                    >
                      <ProgressBarWithInfo
                        nProgress={appState.in_progress?.progress}
                        indeterminate={
                          appState.in_progress?.state ==
                          QueueCompatibilityToolState.Extracting
                        }
                        sOperationText={appState.in_progress?.state}
                        bottomSeparator="none"
                      />
                    </div>
                  )}
                  <Focusable
                    style={{
                      marginLeft: "auto",
                      boxShadow: "none",
                      display: "flex",
                      justifyContent: "right",
                    }}
                  >
                    <DialogButton
                      style={{
                        height: "40px",
                        width: "40px",
                        padding: "10px 12px",
                        minWidth: "40px",
                      }}
                      onClick={(e: MouseEvent) =>
                        showContextMenu(
                          <Menu label="Runner Actions">
                            <MenuItem
                              disabled={isItemInProgress || isQueued}
                              onSelected={() => {}}
                              onClick={() => {
                                handleInstall(release);
                              }}
                            >
                              Install
                            </MenuItem>
                            {(isItemInProgress || isQueued) && (
                              <MenuItem
                                onClick={() => {
                                  handleCancel(release);
                                }}
                              >
                                Cancel from Installation
                              </MenuItem>
                            )}
                            <MenuItem
                              onClick={() => {
                                handleViewChangeLog(release);
                              }}
                            >
                              View Change Log
                            </MenuItem>
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
        </DialogControlsSection>
      )}
    </DialogBody>
  );
}
