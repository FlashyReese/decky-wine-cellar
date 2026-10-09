import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Field,
  Focusable,
  Navigation,
} from "@decky/ui";
import { HiOutlineQrCode } from "react-icons/hi2";
import { SiDiscord, SiGithub, SiKofi } from "react-icons/si";
import { AppState, UpdaterState } from "../types";
import { showQrModal } from "../components/showQrModal";
import { refreshCatalog } from "../utils/backendApi";
import { formatRelativeTime, useTranslation } from "../i18n";

export default function About({
  appState,
  socket,
}: {
  appState: AppState | undefined;
  socket: WebSocket | undefined;
}) {
  const { t } = useTranslation();
  return (
    <DialogBody>
      <DialogControlsSection>
        <div>
          <p>
            {t("about-description")}
          </p>
        </div>
        <DialogControlsSectionHeader>Wine Cellar</DialogControlsSectionHeader>
        <SystemInformation appState={appState} socket={socket} />
        <DialogControlsSectionHeader>
          {t("about-participate")}
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
  const { t, locale } = useTranslation();
  return (
    <Focusable style={{ display: "flex", flexDirection: "column" }}>
      {appState != undefined && socket != undefined && (
        <Field
          label={t("about-updates")}
          description={t("about-lastChecked", {
            time: appState.updater_last_check != null
              ? formatRelativeTime(locale, appState.updater_last_check * 1000)
              : t("about-never"),
          })}
          bottomSeparator={"none"}
        >
          <DialogButton
            disabled={appState.updater_state == UpdaterState.Checking}
            onClick={() => {
              refreshCatalog(socket);
            }}
          >
            {appState.updater_state == UpdaterState.Idle
              ? t("about-checkUpdates")
              : t("about-checking")}
          </DialogButton>
        </Field>
      )}
    </Focusable>
  );
}

function ProjectInformation() {
  const { t } = useTranslation();
  const socialLinks = [
    {
      label: "GitHub",
      icon: <SiGithub size={20} aria-hidden="true" />,
      link: "https://github.com/FlashyReese/decky-wine-cellar",
      buttonText: t("about-reportIssue"),
    },
    {
      label: "Discord",
      icon: <SiDiscord size={20} aria-hidden="true" />,
      link: "https://discord.gg/MPHVG6MH4e",
      buttonText: t("about-join"),
    },
    {
      label: "Ko-fi",
      icon: <SiKofi size={20} aria-hidden="true" />,
      link: "https://ko-fi.com/flashyreese",
      buttonText: t("about-support"),
    },
  ];

  return (
    <Focusable style={{ display: "flex", flexDirection: "column" }}>
      {socialLinks.map((linkInfo, index) => (
        <Field
          key={index}
          label={linkInfo.label}
          icon={linkInfo.icon}
          bottomSeparator={"none"}
          inlineWrap={"shift-children-below"}
          childrenContainerWidth={"min"}
          verticalAlignment={"center"}
        >
          <Focusable
            flow-children="row"
            style={{
              marginLeft: "auto",
              boxShadow: "none",
              display: "flex",
              alignItems: "center",
              justifyContent: "flex-end",
              gap: "8px",
              maxWidth: "100%",
            }}
          >
            <DialogButton
              onClick={() => {
                Navigation.NavigateToExternalWeb(linkInfo.link);
              }}
              style={{
                padding: "10px",
                fontSize: "14px",
                width: "auto",
              }}
            >
              {linkInfo.buttonText}
            </DialogButton>
            <DialogButton
              aria-label={t("about-qrCode", { name: linkInfo.label })}
              onClick={() => {
                showQrModal(linkInfo.link);
              }}
              style={{
                display: "flex",
                justifyContent: "center",
                alignItems: "center",
                padding: 0,
                width: "40px",
                minWidth: "40px",
                height: "40px",
                flexShrink: 0,
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
