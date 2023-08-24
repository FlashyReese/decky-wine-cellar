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

export default function About({
  appState,
  socket,
}: {
  appState: AppState;
  socket: WebSocket;
}) {
  return (
    <DialogBody>
      <DialogControlsSection>
        <div>
          <p>
            Wine Cellar is a compatibility tool manager for Steam. It allows you
            to install, uninstall, and update compatibility tools for Steam
            games.
          </p>
        </div>
        <DialogControlsSectionHeader>Wine Cellar</DialogControlsSectionHeader>
        <SystemInformation appState={appState} socket={socket} />
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
  appState: AppState;
  socket: WebSocket;
}) {
  return (
    <Focusable style={{ display: "flex", flexDirection: "column" }}>
      <Field
        label={"Compatibility Tools Updates"}
        description={
          "Last checked: " +
          (appState.updater_last_check != null
            ? formatDistanceToNow(fromUnixTime(appState.updater_last_check!)) +
              " ago"
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
            }
          }}
        >
          {appState.updater_state == UpdaterState.Idle
            ? "Check For Updates"
            : "Checking..."}
        </DialogButton>
      </Field>
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
