import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Field,
  Focusable,
  Navigation,

} from "decky-frontend-lib";
import { SiDiscord, SiGithub, SiKofi } from "react-icons/si";
import { HiOutlineQrCode } from "react-icons/hi2";
import { showQrModal } from "../components/showQrModal";
import { formatDistanceToNow, fromUnixTime } from "date-fns";
import {
  AppState,
  Request,
  RequestType,
  TaskType,
  UpdaterState,
} from "../types";
import {error, log, warn} from "../utils/logger";
import PortNumberTextField from "../components/PortNumberTextField";
import { BackendCtx } from "../utils/pythonBackendHelper";
import { useEffect, useState } from "react";

export default function About({
  appState,
  socket,
}: {
  appState: AppState | undefined;
  socket: WebSocket | undefined;
}) {
  const [portNumber, setPortNumber] = useState<number>(8887);

  useEffect(() => {
    // Load the initial port number from your backend context
    const loadPortNumber = async () => {
      try {
        const initialPortNumber = (await BackendCtx.getSetting(
          "port",
          8887,
        )) as number;
        setPortNumber(initialPortNumber);
      } catch (error) {
        // Handle the error, e.g., log it or set a default value
        console.error("Error loading port number:", error);
        setPortNumber(8887);
      }
    };

    loadPortNumber().then(() => {
      console.log("Port number loaded");
    });
  }, []);

  return (
    <DialogBody>
      <DialogControlsSection>
        <Field bottomSeparator="none">
          Wine Cellar is a compatibility tool manager for Steam. It allows you to install, uninstall, and update compatibility tools for Steam games.
        </Field>
        <DialogControlsSectionHeader>Wine Cellar Settings</DialogControlsSectionHeader>
        <SystemInformation appState={appState} socket={socket} />
        <Field label={"Wine Cellar Port"}>
          <PortNumberTextField
            value={portNumber + ""}
            onPortNumberChange={(port) => {
              setPortNumber(port);
              log("Port number changed to " + port);
            }}
            rangeMin={1}
            mustBeNumeric={true}
            style={{ minWidth: "80px" }}
          />
        </Field>
        <Field label={"Restart Backend"}>
          <DialogButton
              onClick={() => {
                BackendCtx.restartBackend().then(() => {
                  warn("Backend restarted");
                });
              }}
              style={{
                padding: "10px",
                fontSize: "14px",
              }}
          >
            Restart Now
          </DialogButton>
        </Field>
        <DialogControlsSectionHeader>
          Engage & Participate
        </DialogControlsSectionHeader>
        <ProjectInformation />
      </DialogControlsSection>
    </DialogBody>
  );
}

function SystemInformation({
  appState,
  socket,
}: {
  appState: AppState | undefined;
  socket: WebSocket | undefined;
}) {
  return (
    <Focusable style={{ display: "flex", flexDirection: "column" }}>
      {appState != undefined && socket != undefined && (
        <Field
          label={"Compatibility Tools Updates"}
          description={
            "Last checked: " +
            (appState.updater_last_check != null
              ? formatDistanceToNow(
                  fromUnixTime(appState.updater_last_check!),
                ) + " ago"
              : "Never")
          }
          bottomSeparator={"none"}
        >
          <DialogButton
            disabled={appState.updater_state == UpdaterState.Checking}
            onClick={() => {
              if (socket && socket.readyState === WebSocket.OPEN) {
                const response: Request = {
                  type: RequestType.Task,
                  task: {
                    type: TaskType.CheckForFlavorUpdates,
                  },
                };
                socket.send(JSON.stringify(response));
              } else {
                error("WebSocket not alive...");
              }
            }}
          >
            {appState.updater_state == UpdaterState.Idle
              ? "Check For Updates"
              : "Checking..."}
          </DialogButton>
        </Field>
      )}
    </Focusable>
  );
}

function ProjectInformation() {
  const socialLinks = [
    {
      label: "GitHub",
      icon: <SiGithub />,
      link: "https://github.com/FlashyReese/decky-wine-cellar",
      buttonText: "Report an Issue",
    },
    {
      label: "Discord",
      icon: <SiDiscord />,
      link: "https://discord.gg/MPHVG6MH4e",
      buttonText: "Join Us",
    },
    {
      label: "Ko-fi",
      icon: <SiKofi />,
      link: "https://ko-fi.com/flashyreese",
      buttonText: "Support the Project!",
    },
  ];

  return (
    <Focusable style={{ display: "flex", flexDirection: "column" }}>
      {socialLinks.map((linkInfo, index) => (
        //padding compact is broken lol
        <Field
          key={index}
          label={linkInfo.label}
          icon={linkInfo.icon}
          bottomSeparator={"none"}
          padding={"none"}
        >
          <Focusable
            style={{
              marginLeft: "auto",
              boxShadow: "none",
              display: "flex",
              justifyContent: "right",
              padding: "4px",
            }}
          >
            <DialogButton
              onClick={() => {
                Navigation.NavigateToExternalWeb(linkInfo.link);
              }}
              style={{
                padding: "10px",
                fontSize: "14px",
              }}
            >
              {linkInfo.buttonText}
            </DialogButton>
            <DialogButton
              onClick={() => {
                showQrModal(linkInfo.link);
              }}
              style={{
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                padding: "10px",
                maxWidth: "40px",
                minWidth: "auto",
                marginLeft: ".5em",
              }}
            >
              <HiOutlineQrCode />
            </DialogButton>
          </Focusable>
        </Field>
      ))}
    </Focusable>
  );
}
