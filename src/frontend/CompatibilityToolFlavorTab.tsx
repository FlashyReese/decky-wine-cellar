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
} from "decky-frontend-lib";
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
import { error } from "../logger";
import {Markdown} from "../components/Markdown";

function ChangeLogModal({ release, closeModal }: { release: GitHubRelease; closeModal?: () => {} }) {
  return (
      <Focusable onCancelButton={closeModal}>
          <Focusable
              onActivate={() => {}}
              style={{
                  marginTop: '40px',
                  height: 'calc( 100% - 40px )',
                  overflowY: 'scroll',
                  display: 'flex',
                  justifyContent: 'center',
                  margin: '40px',
              }}
          >
              <div>
                  <h1>{release.name}</h1>
                  {release.body ? (
                      <Markdown>{`${release.body}`}</Markdown>
                  ) : (
                      'no patch notes for this version'
                  )}
              </div>
          </Focusable>
      </Focusable>
  );
}

export default function FlavorTab({
  getAppState,
  getFlavor,
  getSocket,
}: {
  getAppState: AppState;
  getFlavor: Flavor;
  getSocket: WebSocket;
}) {
  const handleInstall = (release: GitHubRelease) => {
    if (getSocket && getSocket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Install,
        install: {
          flavor: getFlavor.flavor,
          release: release,
        },
      };
      getSocket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleUninstall = (release: SteamCompatibilityTool) => {
    if (getSocket && getSocket.readyState === WebSocket.OPEN) {
      const response: Request = {
        type: RequestType.Uninstall,
        uninstall: {
          flavor: getFlavor.flavor,
          steam_compatibility_tool: release, //fixme: we should pass back a directory instead or uuid the backend to use it to find the directory
        },
      };
      getSocket.send(JSON.stringify(response));
    } else {
      error("WebSocket not alive...");
    }
  };

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

  const handleViewChangeLog = (release: GitHubRelease) =>
      showModal(
          <ChangeLogModal release={release}/>
      );

  return (
    <DialogBody>
      {getFlavor.installed.length != 0 && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>Installed</DialogControlsSectionHeader>
          <ul style={{ listStyleType: "none" }}>
            {getFlavor.installed.map((release: SteamCompatibilityTool) => {
              const isQueued = getAppState.in_progress !== null;
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
                    {release.display_name}{" "}
                    {release.requires_restart && "(Requires Restart)"}
                    {release.used_by_games.length != 0 && "(Used By Games)"}
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
                                handleUninstallModal(release);
                              }}
                            >
                              Uninstall
                            </MenuItem>
                            {release.requires_restart && (
                              <MenuItem
                                disabled={isQueued}
                                onClick={() => {
                                  SteamClient.User.StartRestart();
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
      {getFlavor.not_installed.length != 0 && (
        <DialogControlsSection>
          <DialogControlsSectionHeader>
            Not Installed
          </DialogControlsSectionHeader>
          <ul>
            {getFlavor.not_installed.map((release) => {
              const isQueued = getAppState.in_progress !== null;
              const isItemQueued =
                isQueued && getAppState.in_progress?.name === release.tag_name;
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
                    {release.tag_name}{" "}
                    {getAppState.task_queue
                      .filter(
                        (task) =>
                          task.type == TaskType.InstallCompatibilityTool,
                      )
                      .map((task) => task.install)
                      .filter(
                        (install) =>
                          install != null && install.release.url == release.url,
                      ).length == 1 && "(In Queue)"}
                  </span>
                  {isItemQueued && (
                    <div
                      style={{
                        marginLeft: "auto",
                        paddingLeft: "10px",
                        minWidth: "200px",
                      }}
                    >
                      <ProgressBarWithInfo
                        nProgress={getAppState.in_progress?.progress}
                        indeterminate={
                          getAppState.in_progress?.state ==
                          QueueCompatibilityToolState.Extracting
                        }
                        sOperationText={getAppState.in_progress?.state}
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
                              disabled={isItemQueued}
                              onSelected={() => {}}
                              onClick={() => {
                                handleInstall(release);
                              }}
                            >
                              Install
                            </MenuItem>
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
