import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Field,
  Focusable,
} from "decky-frontend-lib";
import { SiDiscord } from "react-icons/si";
import { HiQrCode } from "react-icons/hi2";

export default function About() {
  return (
    <DialogBody>
      <DialogControlsSection>
        <div>
          <p>Wine Cellar is a compatibility tool manager for Steam.</p>
          <p>
            It allows you to install, uninstall, and update compatibility tools
            for Steam games.
          </p>
        </div>
        <DialogControlsSectionHeader>Socials</DialogControlsSectionHeader>
        <ProjectInformation />
      </DialogControlsSection>
    </DialogBody>
  );
}

function ProjectInformation() {
  return (
    <Focusable style={{ display: "flex", alignItems: "center" }}>
      <Field
        label={"Discord"}
        icon={<SiDiscord />}
        childrenContainerWidth={"max"} // Added flex and align-items
      >
        <DialogButton
          onClick={() => {}}
          style={{
            padding: "10px",
            fontSize: "14px",
          }}
        >
          Join Us
        </DialogButton>
        <DialogButton
          onOKActionDescription={"Show QR Code"}
          onClick={() => {}}
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
          <HiQrCode />
        </DialogButton>
      </Field>
    </Focusable>
  );
}
