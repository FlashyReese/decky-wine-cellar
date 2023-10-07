import {
  DialogBody,
  DialogControlsSection,
  Dropdown,
  DropdownOption, Field,
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

    // Add a 500ms delay before calling RequestState, we need to wait for compat tool to be updated
    setTimeout(() => {
      RequestState();
    }, 500);
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
  }, []);

  return (
    <DialogBody>
      <DialogControlsSection>
        <ul style={{ listStyleType: "none" }}>
          {GetInstalledApplications(appState)
            .sort((a, b) => a.name.localeCompare(b.name))
            .map((steamApp: SteamApp) => {
              return (
                <li
                  style={{
                    display: "flex",
                    flexDirection: "row",
                    alignItems: "center",
                    paddingBottom: "10px",
                  }}
                >
                  <Field icon={<img src={steamApp.icon} alt="Icon" />} bottomSeparator={"none"}>{steamApp.name != null ? steamApp.name : steamApp.appId}</Field>
                  {/*<span>
                    {steamApp.name != null ? steamApp.name + " " : steamApp.appId}
                  </span>*/}
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
