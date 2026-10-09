import { toaster, ToastData } from "@decky/api";
import { v4 as uuidv4 } from "uuid";
import { MessageType } from "../types";
import { parseBackendMessage } from "./backendMessages";
import { error, log } from "./logger";
import { translateMessage } from "../i18n";

let shouldReconnect = true;
let socket: WebSocket | null = null;

export const setupToasts = (): void => {
  shouldReconnect = true;
  if (socket != null && socket.readyState !== WebSocket.CLOSED) {
    return;
  }

  const setupWebsocket = (): void => {
    if (!shouldReconnect) {
      return;
    }

    socket = new WebSocket("ws://localhost:8887");
    const uniqueId = uuidv4();

    socket.onopen = (): void => {
      log("WebSocket connection established. Unique Identifier:", uniqueId);
    };

    socket.onmessage = (event: MessageEvent): void => {
      const response = parseBackendMessage(event.data);
      if (response == null) {
        return;
      }

      if (
        response.type === MessageType.Notification &&
        response.notification != null
      ) {
        const toastData: ToastData = {
          title: "Wine Cellar",
          body: translateMessage(response.notification),
          showToast: true,
        };

        toaster.toast(toastData);
        log("Received backend notification:", response.notification);
      }
    };

    socket.onclose = (event: CloseEvent): void => {
      if (shouldReconnect) {
        log(
          "Socket is closed. Unique Identifier:",
          uniqueId,
          "Reconnect will be attempted in 5 seconds.",
          event.reason,
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

    socket.onerror = (event: Event): void => {
      error(
        "Socket encountered error:",
        (event as ErrorEvent).message,
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

export const forceCloseToastsWebSocket = (): void => {
  shouldReconnect = false;
  if (socket) {
    socket.close();
    socket = null;
  }
};
