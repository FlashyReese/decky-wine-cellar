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
  linkInstalledToolToVirtualTool,
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
  const linkableInstalledTools = directInstalledTools.filter(
    (tool) => tool.can_link_to_virtual_tool !== false,
  );
  const operations = [
    ...(appState.current_operation != null ? [appState.current_operation] : []),
    ...appState.queued_operations,
  ];
  const currentTransfer =
    appState.current_operation?.kind === OperationKind.Install ||
    appState.current_operation?.kind === OperationKind.Link
      ? appState.current_operation
      : undefined;

  const showCreateVirtualToolModal = () =>
    showTextPromptModal({
      title: "Create Virtual Tool",
      description:
        "Create a stable compatibility slot, then link an installed tool as its payload. The first time Steam sees the slot, a restart will still be required.",
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

  const handleRemoveVirtualToolModal = (virtualTool: VirtualCompatibilityTool) => {
    const removesLinkedPayload =
      virtualTool.linked_source_installed_tool_id != null;
    const usageWarning =
      virtualTool.used_by_games.length === 0
        ? ""
        : ` ${virtualTool.used_by_games.length} ${virtualTool.used_by_games.length === 1 ? "application is" : "applications are"} assigned to this slot and will need another compatibility tool.`;

    return showModal(
      <ConfirmModal
        strTitle={"Remove " + virtualTool.user_label}
        strDescription={
          removesLinkedPayload
            ? `Removing this virtual compatibility tool deletes the slot and its links. The installed source tool remains installed.${usageWarning}`
            : `Removing this virtual compatibility tool deletes the slot and any payload stored inside it.${usageWarning}`
        }
        strOKButtonText={"Remove"}
        strCancelButtonText={"Cancel"}
        onOK={() => {
          handleRemoveVirtualTool(virtualTool);
        }}
      />,
    );
  };

  const showRenameVirtualToolModal = (virtualTool: VirtualCompatibilityTool) =>
    showTextPromptModal({
      title: "Rename Virtual Tool",
      initialValue: virtualTool.user_label,
      confirmLabel: "Rename",
      onSubmit: (value) => {
        renameVirtualTool(socket, virtualTool.id, value);
      },
    });

  const handleLinkInstalledTool = (
    installedTool: InstalledCompatibilityTool,
    virtualTool: VirtualCompatibilityTool,
  ) => {
    const sourceLabel = getInstalledToolLabel(installedTool);
    const hasPayload = virtualToolHasPayload(virtualTool);
    const usedByCount = virtualTool.used_by_games.length;
    const linkTool = () => {
      linkInstalledToolToVirtualTool(socket, installedTool.id, virtualTool.id);
    };

    if (!hasPayload && usedByCount === 0) {
      linkTool();
      return;
    }

    const details: string[] = [];
    if (hasPayload) {
      details.push(
        `This replaces the current ${getVirtualPayloadLabel(virtualTool)} payload in ${virtualTool.user_label}.`,
      );
    }
    if (usedByCount !== 0) {
      details.push(
        `${usedByCount} ${usedByCount === 1 ? "application is" : "applications are"} assigned to this slot and will use ${sourceLabel} on the next launch.`,
      );
    }
    details.push(
      `${virtualTool.user_label} will depend on the installed ${sourceLabel} files. Keep that source tool installed while the link is in use.`,
    );

    showModal(
      <ConfirmModal
        strTitle={
          hasPayload
            ? `Change payload for ${virtualTool.user_label}`
            : `Link ${sourceLabel} to ${virtualTool.user_label}`
        }
        strDescription={details.join(" ")}
        strOKButtonText="Link Tool"
        strCancelButtonText="Cancel"
        bDestructiveWarning={hasPayload}
        onOK={linkTool}
      />,
    );
  };

  const showInstalledToolPicker = (
    virtualTool: VirtualCompatibilityTool,
    parent: EventTarget,
  ) => {
    const slotBusy = operations.some(
      (operation) => operation.virtual_tool_id === virtualTool.id,
    );

    showContextMenu(
      <Menu label={`Installed tool for ${virtualTool.user_label}`}>
        {linkableInstalledTools.map((installedTool) => {
          const sourceBusy = operations.some(
            (operation) => operation.installed_tool_id === installedTool.id,
          );
          const isCurrentSource =
            virtualTool.linked_source_installed_tool_id === installedTool.id;

          return (
            <MenuItem
              key={installedTool.id}
              disabled={slotBusy || sourceBusy || isCurrentSource}
              onClick={() => {
                handleLinkInstalledTool(installedTool, virtualTool);
              }}
            >
              Link {getInstalledToolLabel(installedTool)}
              {isCurrentSource && " (current)"}
            </MenuItem>
          );
        })}
      </Menu>,
      parent,
      { bFitToWindow: true, bShiftToFitWindow: true },
    );
  };

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
            {currentTransfer.kind === OperationKind.Install && (
              <DialogButton
                style={{ width: "auto", minWidth: "100px", flexShrink: 0 }}
                disabled={currentTransfer.state === OperationState.Cancelling}
                onClick={() => cancelOperation(socket, currentTransfer.id)}
              >
                Cancel
              </DialogButton>
            )}
          </div>
          <OperationProgress operation={currentTransfer} />
        </DialogControlsSection>
      )}
      <DialogControlsSection>
        <DialogControlsSectionHeader>Virtual Tools</DialogControlsSectionHeader>
        <p style={{ marginTop: 0 }}>
          Virtual tools keep a stable Steam-visible slot while linking to an
          installed compatibility tool. The linked source must remain installed.
        </p>
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
              const slotOperation = operations.find(
                (operation) => operation.virtual_tool_id === virtualTool.id,
              );
              const slotOperationIsQueued =
                slotOperation != null &&
                appState.queued_operations.some(
                  (operation) => operation.id === slotOperation.id,
                );
              const slotBusy = slotOperation != null;
              const hasPayload = virtualToolHasPayload(virtualTool);
              const sourcePickerDisabled =
                slotBusy || linkableInstalledTools.length === 0;

              return (
                <li
                  key={virtualTool.id}
                  style={{
                    display: "flex",
                    flexDirection: "row",
                    alignItems: "center",
                    flexWrap: "wrap",
                    gap: "12px",
                    paddingBottom: "10px",
                  }}
                >
                  <div style={{ flex: 1, minWidth: 0, overflowWrap: "anywhere" }}>
                    <div style={{ fontWeight: 600 }}>{virtualTool.user_label}</div>
                    <div style={{ fontSize: "14px", opacity: 0.8 }}>
                      Payload: {getVirtualPayloadLabel(virtualTool)}
                    </div>
                    {virtualTool.linked_source_installed_tool_id != null &&
                      !virtualTool.linked_source_missing && (
                        <div style={{ fontSize: "14px", opacity: 0.8 }}>
                          Linked to an installed source
                        </div>
                      )}
                    {virtualTool.linked_source_missing && (
                      <div
                        role="alert"
                        style={{ fontSize: "14px", color: "#ff9f9f" }}
                      >
                        Linked source is missing. Choose another installed tool.
                      </div>
                    )}
                    {slotOperation != null && (
                      <div role="status" style={{ fontSize: "14px" }}>
                        {slotOperation.state === OperationState.Pending
                          ? "Queued"
                          : slotOperation.state}
                        : {slotOperation.label}
                      </div>
                    )}
                    {virtualTool.requires_restart && (
                      <div style={{ fontSize: "14px", color: "#f5c56b" }}>
                        Restart Steam to make this slot available
                      </div>
                    )}
                    {virtualTool.used_by_games.length !== 0 && (
                      <div style={{ fontSize: "14px", opacity: 0.8 }}>
                        Assigned to {virtualTool.used_by_games.length}{" "}
                        {virtualTool.used_by_games.length === 1
                          ? "application"
                          : "applications"}
                      </div>
                    )}
                  </div>
                  <Focusable
                    flow-children="row"
                    style={{
                      marginLeft: "auto",
                      flexShrink: 0,
                      boxShadow: "none",
                      display: "flex",
                      alignItems: "center",
                      gap: "8px",
                    }}
                  >
                    {slotOperationIsQueued && slotOperation != null && (
                      <DialogButton
                        style={{
                          height: "40px",
                          width: "auto",
                          minWidth: "88px",
                          padding: "8px 12px",
                        }}
                        onClick={() => cancelOperation(socket, slotOperation.id)}
                      >
                        Cancel
                      </DialogButton>
                    )}
                    <DialogButton
                      disabled={sourcePickerDisabled}
                      focusable={!sourcePickerDisabled}
                      style={{
                        height: "40px",
                        width: "auto",
                        minWidth: "132px",
                        padding: "8px 12px",
                      }}
                      onClick={(event: MouseEvent) => {
                        showInstalledToolPicker(
                          virtualTool,
                          event.currentTarget ?? window,
                        );
                      }}
                    >
                      {linkableInstalledTools.length === 0
                        ? "No Linkable Tools"
                        : hasPayload
                          ? "Change Payload"
                          : "Link Installed Tool"}
                    </DialogButton>
                    <DialogButton
                      aria-label={`Actions for virtual tool ${virtualTool.user_label}`}
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

      <DialogControlsSection>
        <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
        {directInstalledTools.length === 0 ? (
          <div>No installed compatibility tools are available to link.</div>
        ) : (
          <ul style={{ listStyleType: "none", margin: 0, padding: 0 }}>
            {directInstalledTools.map((installedTool) => {
              const linkedSlots = appState.virtual_tools.filter(
                (virtualTool) =>
                  virtualTool.linked_source_installed_tool_id === installedTool.id,
              );
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
                    {linkedSlots.length !== 0 &&
                      ` (Linked to ${linkedSlots.map((slot) => slot.user_label).join(", ")})`}
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
                            {installedTool.can_link_to_virtual_tool === false && (
                              <MenuItem disabled>
                                Link unavailable (source folder is a symlink)
                              </MenuItem>
                            )}
                            {appState.virtual_tools.length === 0 && (
                              <MenuItem disabled>
                                Create a virtual tool before linking
                              </MenuItem>
                            )}
                            {installedTool.can_link_to_virtual_tool !== false &&
                              appState.virtual_tools.map((virtualTool) => {
                              const linkBusy = operations.some(
                                (operation) =>
                                  operation.virtual_tool_id === virtualTool.id ||
                                  operation.installed_tool_id === installedTool.id,
                              );
                              const isCurrentSource =
                                virtualTool.linked_source_installed_tool_id ===
                                installedTool.id;

                              return (
                                <MenuItem
                                  key={virtualTool.id}
                                  disabled={linkBusy || isCurrentSource}
                                  onClick={() => {
                                    handleLinkInstalledTool(
                                      installedTool,
                                      virtualTool,
                                    );
                                  }}
                                >
                                  Link to {virtualTool.user_label}
                                  {isCurrentSource
                                    ? " (current)"
                                    : virtualToolHasPayload(virtualTool) &&
                                      " (replaces payload)"}
                                </MenuItem>
                              );
                              })}
                            <MenuItem
                              disabled={
                                installedToolBusy || linkedSlots.length !== 0
                              }
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
  return tool.user_label ?? tool.display_name;
}

function virtualToolHasPayload(tool: VirtualCompatibilityTool): boolean {
  return (
    tool.current_payload_name != null ||
    tool.current_payload_release_id != null ||
    tool.linked_source_installed_tool_id != null
  );
}

function getVirtualPayloadLabel(tool: VirtualCompatibilityTool): string {
  const payloadLabel =
    tool.current_payload_name ??
    (tool.current_payload_release_id != null ? "Installed payload" : "Empty");
  return payloadLabel;
}
