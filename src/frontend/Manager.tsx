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
} from "decky-frontend-lib";
import { FaEllipsisH } from "react-icons/fa";
import {
  AppState,
  CompatibilityToolFlavor,
  Request,
  RequestType,
  SteamCompatibilityTool,
} from "../types";
import { error } from "../logger";

export default function ManagerTab({
  getAppState,
  getSocket,
}: {
  getAppState: AppState;
  getSocket: WebSocket;
}) {
  const handleUninstall = (release: SteamCompatibilityTool) => {
    if (getSocket && getSocket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Uninstall,
        uninstall: {
          flavor: CompatibilityToolFlavor.Unknown,
          steam_compatibility_tool: release, //fixme: we should pass back a directory instead or uuid the backend to use it to find the directory
        },
      };
      getSocket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleViewUsedByGames = (release: SteamCompatibilityTool) => {
    showModal(
        <ConfirmModal
            strTitle={"Steam Applications using " + release.display_name}
            strDescription={
                release.used_by_games.join("\n")
            }
            strOKButtonText={"OK"}
        />,
    );
  }

  const handleUninstallModal = (release: SteamCompatibilityTool) =>
    showModal(
      <ConfirmModal
        strTitle={"Uninstallation of " + release.display_name}
        strDescription={
          "Are you sure want to remove this compatibility tool? Used by " +
          release.used_by_games.join(",")
        }
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
          {getAppState.installed_compatibility_tools.map(
            (steam_compatibility_tool) => {
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
                    {steam_compatibility_tool.display_name}
                    {steam_compatibility_tool.requires_restart && ("(Requires Restart)")}
                    {steam_compatibility_tool.used_by_games.length != 0 && ("(In Use)")}
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
                                handleUninstallModal(steam_compatibility_tool);
                              }}
                            >
                              Uninstall
                            </MenuItem>
                            {steam_compatibility_tool.used_by_games.length != 0 && (
                                <MenuItem
                                    onSelected={() => {}}
                                    onClick={() => {
                                      handleViewUsedByGames(steam_compatibility_tool);
                                    }}
                                >
                                  View Used By Games
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
