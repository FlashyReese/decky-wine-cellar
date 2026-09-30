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
  CatalogRelease,
  Flavor,
  InstalledCompatibilityTool,
  OperationKind,
  OperationState,
} from "../types";
import {
  cancelOperation,
  installCatalogRelease,
  uninstallInstalledTool,
} from "../utils/backendApi";
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
  const installedToolsForFlavor = appState.installed_tools.filter(
    (tool) => tool.flavor === flavor.flavor,
  );
  const operations = [
    ...(appState.current_operation != null ? [appState.current_operation] : []),
    ...appState.queued_operations,
  ];

  const handleViewUsedByGames = (tool: InstalledCompatibilityTool) => {
    showModal(
      <ConfirmModal
        strTitle={"Steam applications using " + getToolLabel(tool)}
        strDescription={tool.used_by_games.join(", ")}
        strOKButtonText={"OK"}
      />,
    );
  };

  const handleViewChangeLog = (release: CatalogRelease) => {
    showModal(<ChangeLogModal release={release.release} />);
  };

  const handleUninstallToolModal = (tool: InstalledCompatibilityTool) =>
    showModal(
      <ConfirmModal
        strTitle={"Remove " + getToolLabel(tool)}
        strDescription={"Are you sure you want to remove this compatibility tool?"}
        strOKButtonText={"Remove"}
        strCancelButtonText={"Cancel"}
        onOK={() => {
          uninstallInstalledTool(socket, tool.id);
        }}
      />,
    );

  return (
    <DialogBody>
      {installedToolsForFlavor.length !== 0 && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
          <ul style={{ listStyleType: "none", margin: 0, padding: 0 }}>
            {installedToolsForFlavor.map((tool) => {
              const toolBusy = operations.some(
                (operation) => operation.installed_tool_id === tool.id,
              );

              return (
                <li
                  key={tool.id}
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
                    {getToolLabel(tool)}
                    {tool.requires_restart && " (Requires Restart)"}
                    {tool.used_by_games.length !== 0 && " (Used By Games)"}
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
                      aria-label={`Actions for installed tool ${getToolLabel(tool)}`}
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
                              disabled={toolBusy}
                              onClick={() => {
                                handleUninstallToolModal(tool);
                              }}
                            >
                              Remove
                            </MenuItem>
                            {tool.used_by_games.length !== 0 && (
                              <MenuItem
                                onClick={() => {
                                  handleViewUsedByGames(tool);
                                }}
                              >
                                View Used By Games
                              </MenuItem>
                            )}
                            {tool.github_release != null && (
                              <MenuItem
                                onClick={() => {
                                  if (tool.catalog_release_id != null) {
                                    const release = flavor.releases.find(
                                      (catalogRelease) =>
                                        catalogRelease.id === tool.catalog_release_id,
                                    );
                                    if (release != null) {
                                      handleViewChangeLog(release);
                                    }
                                  }
                                }}
                              >
                                View Change Log
                              </MenuItem>
                            )}
                            {tool.requires_restart && (
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
        </DialogControlsSection>
      )}

      <DialogControlsSection>
        <DialogControlsSectionHeader>Catalog</DialogControlsSectionHeader>
        <ul style={{ listStyleType: "none", margin: 0, padding: 0 }}>
          {flavor.releases.map((release) => {
            const isInstalled = appState.installed_tools.some(
              (tool) => tool.catalog_release_id === release.id,
            );
            const releaseOperations = operations.filter(
              (operation) =>
                operation.kind === OperationKind.Install &&
                operation.release_id === release.id,
            );
            const activeInstallOperation =
              appState.current_operation != null &&
              appState.current_operation.release_id === release.id &&
              appState.current_operation.kind === OperationKind.Install
                ? appState.current_operation
                : undefined;
            const installBusy = releaseOperations.length !== 0;

            return (
              <li
                key={release.id}
                style={{ paddingBottom: "16px", minWidth: 0 }}
              >
                <div
                  style={{
                    display: "flex",
                    alignItems: "center",
                    minWidth: 0,
                  }}
                >
                  <span style={{ minWidth: 0, overflowWrap: "anywhere" }}>
                    {release.release.tag_name}
                    {isInstalled && " (Installed)"}
                    {releaseOperations.some(
                      (operation) => operation.state === OperationState.Pending,
                    ) && " (Queued)"}
                  </span>
                  <Focusable
                    style={{
                      marginLeft: "auto",
                      flexShrink: 0,
                      paddingLeft: "12px",
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
                      onClick={(event: MouseEvent) =>
                        showContextMenu(
                          <Menu label="Catalog Release Actions">
                            <MenuItem
                              disabled={isInstalled || installBusy}
                              onClick={() => {
                                installCatalogRelease(socket, release.id);
                              }}
                            >
                              Install
                            </MenuItem>

                            {releaseOperations.map((operation) => (
                              <MenuItem
                                key={operation.id}
                                disabled={
                                  operation.state === OperationState.Cancelling
                                }
                                onClick={() => {
                                  cancelOperation(socket, operation.id);
                                }}
                              >
                                Cancel {operation.label}
                              </MenuItem>
                            ))}
                            <MenuItem
                              onClick={() => {
                                handleViewChangeLog(release);
                              }}
                            >
                              View Change Log
                            </MenuItem>
                          </Menu>,
                          event.currentTarget ?? window,
                        )
                      }
                    >
                      <FaEllipsisH />
                    </DialogButton>
                  </Focusable>
                </div>
                {activeInstallOperation != null && (
                  <OperationProgress operation={activeInstallOperation} />
                )}
              </li>
            );
          })}
        </ul>
      </DialogControlsSection>
    </DialogBody>
  );
}

function getToolLabel(tool: InstalledCompatibilityTool): string {
  return tool.display_name;
}
