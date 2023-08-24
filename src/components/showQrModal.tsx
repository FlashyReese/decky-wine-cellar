import { ModalRoot, showModal } from "decky-frontend-lib";
import QRCode from "react-qr-code";

export const showQrModal = (url: string) => {
  showModal(
    <ModalRoot>
      <div
        style={{
          margin: "0 auto 1.5em auto",
          padding: "1em", // Add padding for whitespace
          borderRadius: "2em", // Add rounded corners
          background: "#F5F5F5", // Light gray background color
          boxShadow: "0 1em 2em rgba(0, 0, 0, 0.5)", // Dark gray shadow color
        }}
      >
        <QRCode value={url} size={256} fgColor="#000000" bgColor="#F5F5F5" />
      </div>
      <span style={{ textAlign: "center", wordBreak: "break-word" }}>
        {url}
      </span>
    </ModalRoot>,
    window,
  );
};
