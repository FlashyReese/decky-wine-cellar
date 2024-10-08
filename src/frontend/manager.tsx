import {
  ConfirmModal,
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Focusable,
  Menu,
  MenuItem,
  showContextMenu,
  showModal,
} from "@decky/ui";
import { FaEllipsisH } from "react-icons/fa";
import {
  AppState,
  CompatibilityToolFlavor,
  GitHubRelease,
  Request,
  RequestType,
  SteamCompatibilityTool,
  TaskType,
} from "../types";
import { error } from "../utils/logger";
import { RestartSteamClient } from "../utils/steamUtils";
import ChangeLogModal from "../components/changeLogModal";

export default function ManagerTab({
  appState,
  socket,
}: {
  appState: AppState;
  socket: WebSocket;
}) {
  const handleUninstall = (release: SteamCompatibilityTool) => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Task,
        task: {
          type: TaskType.UninstallCompatibilityTool,
          uninstall: {
            flavor: CompatibilityToolFlavor.Unknown,
            steam_compatibility_tool: release,
          },
        },
      };
      socket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleViewUsedByGames = (release: SteamCompatibilityTool) => {
    showModal(
      <ConfirmModal
        strTitle={"Steam Applications using " + release.display_name}
        strDescription={release.used_by_games.join(", ")}
        strOKButtonText={"OK"}
      />,
    );
  };

  const handleViewChangeLog = (release: GitHubRelease) =>
    showModal(<ChangeLogModal release={release} />);

  const handleUninstallModal = (release: SteamCompatibilityTool) =>
    showModal(
      <ConfirmModal
        strTitle={"Uninstallation of " + release.display_name}
        strDescription={"Are you sure want to remove this compatibility tool?"}
        strOKButtonText={"Uninstall"}
        strCancelButtonText={"Cancel"}
        onOK={() => {
          handleUninstall(release);
        }}
      />,
    );

  return (
    <DialogBody>
      <DialogControlsSection>
        <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
        <ul>
          {appState.installed_compatibility_tools.map(
            (steamCompatibilityTool: SteamCompatibilityTool) => {
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
                    {steamCompatibilityTool.display_name}
                    {steamCompatibilityTool.requires_restart &&
                      " (Requires Restart)"}
                    {steamCompatibilityTool.used_by_games.length != 0 &&
                      " (Used By Games)"}
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
                              onSelected={() => {}}
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
                                  handleViewUsedByGames(steamCompatibilityTool);
                                }}
                              >
                                View Used By Games
                              </MenuItem>
                            )}
                            {steamCompatibilityTool.github_release != null && (
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
                                disabled={
                                  !steamCompatibilityTool.requires_restart
                                }
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
            },
          )}
        </ul>
      </DialogControlsSection>
    </DialogBody>
  );
}
