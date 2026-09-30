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
import ChangeLogModal from "../components/changeLogModal";
import OperationProgress from "../components/operationProgress";
import {
  AppState,
  GitHubRelease,
  InstalledCompatibilityTool,
  OperationKind,
  OperationState,
} from "../types";
import { cancelOperation, uninstallInstalledTool } from "../utils/backendApi";
import { RestartSteamClient } from "../utils/steamUtils";

export default function ManagerTab({
  appState,
  socket,
}: {
  appState: AppState;
  socket: WebSocket;
}) {
  const installedTools = appState.installed_tools;

  const operations = [
    ...(appState.current_operation != null ? [appState.current_operation] : []),
    ...appState.queued_operations,
  ];
  const currentTransfer =
    appState.current_operation?.kind === OperationKind.Install
      ? appState.current_operation
      : undefined;

  const handleViewUsedByGames = (title: string, usedByGames: string[]) => {
    showModal(
      <ConfirmModal
        strTitle={"Steam applications using " + title}
        strDescription={usedByGames.join(", ")}
        strOKButtonText={"OK"}
      />,
    );
  };

  const handleViewChangeLog = (release: GitHubRelease) => {
    showModal(<ChangeLogModal release={release} />);
  };

  const handleRemoveInstalledTool = (tool: InstalledCompatibilityTool) => {
    uninstallInstalledTool(socket, tool.id);
  };

  const handleRemoveInstalledToolModal = (tool: InstalledCompatibilityTool) =>
    showModal(
      <ConfirmModal
        strTitle={"Remove " + getInstalledToolLabel(tool)}
        strDescription={"Are you sure you want to remove this compatibility tool?"}
        strOKButtonText={"Remove"}
        strCancelButtonText={"Cancel"}
        onOK={() => {
          handleRemoveInstalledTool(tool);
        }}
      />,
    );

  return (
    <DialogBody>
      {currentTransfer != null && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>
            Current Operation
          </DialogControlsSectionHeader>
          <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
            <span style={{ flex: 1, minWidth: 0, overflowWrap: "anywhere" }}>
              {currentTransfer.label}
            </span>
            <DialogButton
              style={{ width: "auto", minWidth: "100px", flexShrink: 0 }}
              disabled={currentTransfer.state === OperationState.Cancelling}
              onClick={() => cancelOperation(socket, currentTransfer.id)}
            >
              Cancel
            </DialogButton>
          </div>
          <OperationProgress operation={currentTransfer} />
        </DialogControlsSection>
      )}

      <DialogControlsSection>
        <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
        {installedTools.length === 0 ? (
          <div>No installed compatibility tools are available.</div>
        ) : (
          <ul style={{ listStyleType: "none", margin: 0, padding: 0 }}>
            {installedTools.map((installedTool) => {
              const installedToolBusy = operations.some(
                (operation) => operation.installed_tool_id === installedTool.id,
              );

              return (
                <li
                  key={installedTool.id}
                  style={{
                    display: "flex",
                    flexDirection: "row",
                    alignItems: "center",
                    flexWrap: "wrap",
                    gap: "12px",
                    paddingBottom: "10px",
                  }}
                >
                  <span style={{ flex: 1, minWidth: 0, overflowWrap: "anywhere" }}>
                    {getInstalledToolLabel(installedTool)}
                    {installedTool.requires_restart && " (Requires Restart)"}
                    {installedTool.used_by_games.length !== 0 &&
                      " (Used By Games)"}
                  </span>
                  <Focusable
                    style={{
                      marginLeft: "auto",
                      flexShrink: 0,
                      boxShadow: "none",
                      display: "flex",
                      justifyContent: "flex-end",
                    }}
                  >
                    <DialogButton
                      aria-label={`Actions for installed tool ${getInstalledToolLabel(installedTool)}`}
                      style={{
                        height: "40px",
                        width: "40px",
                        padding: "10px 12px",
                        minWidth: "40px",
                      }}
                      onClick={(event: MouseEvent) =>
                        showContextMenu(
                          <Menu label="Installed Tool Actions">

                            <MenuItem
                              disabled={installedToolBusy}
                              onClick={() => {
                                handleRemoveInstalledToolModal(installedTool);
                              }}
                            >
                              Remove
                            </MenuItem>
                            {installedTool.used_by_games.length !== 0 && (
                              <MenuItem
                                onClick={() => {
                                  handleViewUsedByGames(
                                    getInstalledToolLabel(installedTool),
                                    installedTool.used_by_games,
                                  );
                                }}
                              >
                                View Used By Games
                              </MenuItem>
                            )}
                            {installedTool.github_release != null && (
                              <MenuItem
                                onClick={() => {
                                  if (installedTool.github_release != null) {
                                    handleViewChangeLog(installedTool.github_release);
                                  }
                                }}
                              >
                                View Change Log
                              </MenuItem>
                            )}
                            {installedTool.requires_restart && (
                              <MenuItem
                                onClick={() => {
                                  RestartSteamClient();
                                }}
                              >
                                Restart Steam
                              </MenuItem>
                            )}
                          </Menu>,
                          event.currentTarget ?? window,
                          { bFitToWindow: true, bShiftToFitWindow: true },
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
        )}
      </DialogControlsSection>
    </DialogBody>
  );
}

function getInstalledToolLabel(tool: InstalledCompatibilityTool): string {
  return tool.display_name;
}
