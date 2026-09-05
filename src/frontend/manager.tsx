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
import { showTextPromptModal } from "../components/textPromptModal";
import {
  AppState,
  GitHubRelease,
  InstalledCompatibilityTool,
  InstalledToolSource,
  OperationKind,
  OperationState,
  VirtualCompatibilityTool,
} from "../types";
import {
  cancelOperation,
  createVirtualTool,
  removeVirtualTool,
  renameVirtualTool,
  uninstallInstalledTool,
} from "../utils/backendApi";
import { RestartSteamClient } from "../utils/steamUtils";

export default function ManagerTab({
  appState,
  socket,
}: {
  appState: AppState;
  socket: WebSocket;
}) {
  const directInstalledTools = appState.installed_tools.filter(
    (tool) => tool.source !== InstalledToolSource.Virtual,
  );
  const operations = [
    ...(appState.current_operation != null ? [appState.current_operation] : []),
    ...appState.queued_operations,
  ];
  const currentInstall =
    appState.current_operation?.kind === OperationKind.Install
      ? appState.current_operation
      : undefined;

  const showCreateVirtualToolModal = () =>
    showTextPromptModal({
      title: "Create Virtual Tool",
      description:
        "Create a stable compatibility slot. The first time Steam sees it, a restart will still be required.",
      confirmLabel: "Create",
      onSubmit: (value) => {
        createVirtualTool(socket, value);
      },
    });

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

  const handleRemoveVirtualTool = (virtualTool: VirtualCompatibilityTool) => {
    if (virtualTool.installed_tool_id != null) {
      uninstallInstalledTool(socket, virtualTool.installed_tool_id);
      return;
    }

    removeVirtualTool(socket, virtualTool.id);
  };

  const handleRemoveVirtualToolModal = (virtualTool: VirtualCompatibilityTool) =>
    showModal(
      <ConfirmModal
        strTitle={"Remove " + virtualTool.user_label}
        strDescription={
          "Removing a virtual compatibility tool deletes the slot and any mounted payload."
        }
        strOKButtonText={"Remove"}
        strCancelButtonText={"Cancel"}
        onOK={() => {
          handleRemoveVirtualTool(virtualTool);
        }}
      />,
    );

  const showRenameVirtualToolModal = (virtualTool: VirtualCompatibilityTool) =>
    showTextPromptModal({
      title: "Rename Virtual Tool",
      initialValue: virtualTool.user_label,
      confirmLabel: "Rename",
      onSubmit: (value) => {
        renameVirtualTool(socket, virtualTool.id, value);
      },
    });

  return (
    <DialogBody>
      {currentInstall != null && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>
            Current Installation
          </DialogControlsSectionHeader>
          <div style={{ display: "flex", alignItems: "center", gap: "12px" }}>
            <span style={{ flex: 1, minWidth: 0, overflowWrap: "anywhere" }}>
              {currentInstall.label}
            </span>
            <DialogButton
              style={{ width: "auto", minWidth: "100px", flexShrink: 0 }}
              disabled={currentInstall.state === OperationState.Cancelling}
              onClick={() => cancelOperation(socket, currentInstall.id)}
            >
              Cancel
            </DialogButton>
          </div>
          <OperationProgress operation={currentInstall} />
        </DialogControlsSection>
      )}
      <DialogControlsSection>
        <DialogControlsSectionHeader>Virtual Tools</DialogControlsSectionHeader>
        <DialogButton onClick={showCreateVirtualToolModal}>
          Create Virtual Tool
        </DialogButton>
        {appState.virtual_tools.length === 0 ? (
          <div style={{ paddingTop: "10px" }}>
            No virtual tools yet. Create one to reuse a stable Steam-visible slot
            without restarting for every payload change.
          </div>
        ) : (
          <ul style={{ listStyleType: "none", margin: 0, padding: 0 }}>
            {appState.virtual_tools.map((virtualTool) => {
              const slotBusy = operations.some(
                (operation) => operation.virtual_tool_id === virtualTool.id,
              );

              return (
                <li
                  key={virtualTool.id}
                  style={{
                    display: "flex",
                    flexDirection: "row",
                    alignItems: "center",
                    paddingBottom: "10px",
                  }}
                >
                  <span style={{ minWidth: 0, overflowWrap: "anywhere" }}>
                    {virtualTool.user_label}
                    {virtualTool.current_payload_name != null &&
                      " (" + virtualTool.current_payload_name + ")"}
                    {virtualTool.current_payload_name == null && " (Empty)"}
                    {virtualTool.requires_restart && " (Requires Restart)"}
                    {virtualTool.used_by_games.length !== 0 && " (Used By Games)"}
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
                          <Menu label="Virtual Tool Actions">
                            <MenuItem
                              disabled={slotBusy}
                              onClick={() => {
                                showRenameVirtualToolModal(virtualTool);
                              }}
                            >
                              Rename
                            </MenuItem>
                            <MenuItem
                              disabled={slotBusy}
                              onClick={() => {
                                handleRemoveVirtualToolModal(virtualTool);
                              }}
                            >
                              Remove
                            </MenuItem>
                            {virtualTool.used_by_games.length !== 0 && (
                              <MenuItem
                                onClick={() => {
                                  handleViewUsedByGames(
                                    virtualTool.user_label,
                                    virtualTool.used_by_games,
                                  );
                                }}
                              >
                                View Used By Games
                              </MenuItem>
                            )}
                            {virtualTool.github_release != null && (
                              <MenuItem
                                onClick={() => {
                                  if (virtualTool.github_release != null) {
                                    handleViewChangeLog(virtualTool.github_release);
                                  }
                                }}
                              >
                                View Current Payload Change Log
                              </MenuItem>
                            )}
                            {virtualTool.requires_restart && (
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

      <DialogControlsSection>
        <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
        <ul style={{ listStyleType: "none", margin: 0, padding: 0 }}>
          {directInstalledTools.map((installedTool) => (
            <li
              key={installedTool.id}
              style={{
                display: "flex",
                flexDirection: "row",
                alignItems: "center",
                paddingBottom: "10px",
              }}
            >
              <span style={{ minWidth: 0, overflowWrap: "anywhere" }}>
                {getInstalledToolLabel(installedTool)}
                {installedTool.requires_restart && " (Requires Restart)"}
                {installedTool.used_by_games.length !== 0 && " (Used By Games)"}
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
                      <Menu label="Installed Tool Actions">
                        <MenuItem
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
                    )
                  }
                >
                  <FaEllipsisH />
                </DialogButton>
              </Focusable>
            </li>
          ))}
        </ul>
      </DialogControlsSection>
    </DialogBody>
  );
}

function getInstalledToolLabel(tool: InstalledCompatibilityTool): string {
  return tool.user_label ?? tool.display_name;
}
