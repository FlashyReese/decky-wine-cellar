import { ServerAPI, ToastData } from "decky-frontend-lib";
import { log, error } from "./logger";
import { Request, RequestType } from "../types";
import { v4 as uuidv4 } from "uuid"; // Import UUID v4

let shouldReconnect = true; // Global flag to control reconnection
let socket: WebSocket | null = null; // Global WebSocket reference

export const setupToasts = (serverAPI: ServerAPI): void => {
  const setupWebsocket = (): void => {
    if (!shouldReconnect) {
      return; // If reconnection is disabled, don't proceed
    }

    socket = new WebSocket("ws://localhost:8887");
    const uniqueId = uuidv4(); // Generate a unique identifier using UUID

    socket.onopen = (): void => {
      log("WebSocket connection established. Unique Identifier: ", uniqueId);
    };

    socket.onmessage = (e: MessageEvent): void => {
      const response: Request = JSON.parse(e.data);
      if (response.type == RequestType.Notification) {
        if (response.notification != null && response.notification != "") {
          let toastData: ToastData = {
            title: "Wine Cellar",
            body: response.notification,
            showToast: true,
          };

          serverAPI.toaster.toast(toastData);
          log("Received backend notification: " + response.notification);
        }
      }
      console.log(JSON.stringify(response));
    };

    socket.onclose = (e: CloseEvent): void => {
      if (shouldReconnect) {
        log(
          "Socket is closed. Unique Identifier:",
          uniqueId,
          "Reconnect will be attempted in 5 seconds.",
          e.reason,
        );
        setTimeout(() => {
          setupWebsocket();
        }, 5000);
      } else {
        log(
          "Socket is closed. Reconnection is disabled. Unique Identifier:",
          uniqueId,
        );
      }
    };

    socket.onerror = (err: Event): void => {
      error(
        "Socket encountered error: ",
        (err as ErrorEvent).message,
        "Unique Identifier:",
        uniqueId,
      );
      if (socket) {
        socket.close();
      }
    };
  };

  setupWebsocket();
};

// Function to forcibly close the WebSocket and prevent reconnection
export const forceCloseToastsWebSocket = (): void => {
  shouldReconnect = false;
  if (socket) {
    socket.close();
  }
};
