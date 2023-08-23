import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Field,
  Focusable,
  ModalRoot,
  Navigation,
  showModal,
} from "decky-frontend-lib";
import { SiDiscord, SiGithub, SiKofi } from "react-icons/si";
import { HiOutlineQrCode } from "react-icons/hi2";
import QRCode from "react-qr-code";

export default function About() {
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
        <DialogControlsSectionHeader>
          Engage & Participate
        </DialogControlsSectionHeader>
        <ProjectInformation />
      </DialogControlsSection>
    </DialogBody>
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

export const showQrModal = (url: string) => {
  showModal(
    <ModalRoot>
      <div
        style={{
          margin: "0 auto 1.5em auto",
          padding: "1em", // Add padding for whitespace
          borderRadius: "10px", // Add rounded corners
          background: "#FFFFFF", // Optional: Set background color
          boxShadow: "0 4px 8px rgba(0, 0, 0, 0.1)", // Optional: Add shadow
        }}
      >
        <QRCode value={url} size={256} fgColor="#000000" bgColor="#FFFFFF" />
      </div>
      <span style={{ textAlign: "center", wordBreak: "break-word" }}>
        {url}
      </span>
    </ModalRoot>,
    window,
  );
};
