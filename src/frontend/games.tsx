import {
  DialogBody,
  DialogControlsSection,
  Dropdown,
  DropdownOption,
  Focusable,
} from "decky-frontend-lib";
import { AppState, Request, RequestType } from "../types";
import {
  GetGlobalCompatTools,
  GetInstalledApplications,
  SpecifyCompatTool,
  SteamApp,
} from "../utils/steamUtils";
import { useEffect, useState } from "react";
import { error } from "../utils/logger";

export default function GamesTab({
  appState,
  socket,
}: {
  appState: AppState;
  socket: WebSocket;
}) {
  const [dropdownOptions, setDropdownOptions] = useState<DropdownOption[]>([]);
  const [installedApplications, setInstalledApplications] = useState<
    SteamApp[]
  >([]);

  const RequestState = () => {
    if (socket && socket.readyState === WebSocket.OPEN) {
      GetGlobalCompatTools()
        .then((tools) => {
          const response: Request = {
            type: RequestType.RequestState,
            available_compat_tools: tools,
          };

          socket.send(JSON.stringify(response));
        })
        .catch((err) => error(err));
    } else {
      error("WebSocket not alive...");
    }
  };

  const handleDropdownChange = (
    steamApp: SteamApp,
    selectedOption: DropdownOption,
  ) => {
    SpecifyCompatTool(steamApp.appId, selectedOption.data);
    RequestState();
    // Recall GetInstalledApplications to update the list after the change
    fetchInstalledApplications();
  };

  const fetchInstalledApplications = () => {
    const installedApps = GetInstalledApplications(appState);
    console.log(JSON.stringify(installedApps));
    // Update state with the new installed applications
    setInstalledApplications(installedApps);
  };

  useEffect(() => {
    GetGlobalCompatTools()
      .then((tools) => {
        // None option
        let options: DropdownOption[] = [
          {
            data: "",
            label: "None",
          } as DropdownOption,
        ];
        tools
          .map((t) => {
            return {
              data: t.strToolName,
              label: t.strDisplayName,
            } as DropdownOption;
          })
          .forEach((t) => {
            options.push(t);
          });
        setDropdownOptions(options);
      })
      .catch((err) => error(err));
    fetchInstalledApplications();
  }, []);

  return (
    <DialogBody>
      <DialogControlsSection>
        <ul style={{ listStyleType: "none" }}>
          {installedApplications.map((steamApp: SteamApp) => {
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
                  {steamApp.name != null ? steamApp.name : steamApp.appId}
                </span>
                <Focusable
                  style={{
                    marginLeft: "auto",
                    boxShadow: "none",
                    display: "flex",
                    justifyContent: "right",
                  }}
                >
                  <Dropdown
                    rgOptions={dropdownOptions}
                    selectedOption={steamApp.specified_tool}
                    onChange={(change) =>
                      handleDropdownChange(steamApp, change)
                    }
                  />
                </Focusable>
              </li>
            );
          })}
        </ul>
      </DialogControlsSection>
    </DialogBody>
  );
}
