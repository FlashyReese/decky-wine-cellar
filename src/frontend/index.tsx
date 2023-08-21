import { SidebarNavigation, SidebarNavigationPage } from "decky-frontend-lib";

import { useEffect, useState } from "react";
import { AppState, Request, RequestType } from "../types";
import { log } from "../logger";
import { v4 as uuidv4 } from "uuid";
import FlavorTab from "./CompatibilityToolFlavorTab";
import VirtualCompatibilityTools from "./VirtualCompatibilityTools";

export default function ManagePage() {
  const [appState, setAppState] = useState<AppState | null>(null);

  const [socket, setSocket] = useState<WebSocket>();

  useEffect(() => {
    const socket = new WebSocket("ws://localhost:8887");
    const uniqueId = uuidv4(); // Generate a unique identifier

    setSocket(socket);

    socket.onopen = async () => {
      log("WebSocket connection established. Unique Identifier:", uniqueId); // Log the unique identifier on connection open
      const response: Request = {
        type: RequestType.RequestState,
      };
      socket.send(JSON.stringify(response));
    };

    socket.onmessage = async (event) => {
      log("Received message from server:", event.data);
      const response: Request = JSON.parse(event.data);
      if (response.type == RequestType.UpdateState) {
        if (response.app_state != null) {
          setAppState(response.app_state);
          console.log(response.app_state);
          //log("Received app state update");
        }
      }
    };

    socket.onerror = (error) => {
      log("WebSocket error:", error);
    };

    socket.onclose = () => {
      log("WebSocket connection closed. Unique Identifier:", uniqueId); // Log the unique identifier on connection close
    };

    return () => {
      socket.close(); // Close the WebSocket connection on component unmount
    };
  }, []);

  const pages: (SidebarNavigationPage | "separator")[] = [];

  if (appState != null && socket != null) {
    //const flavor_pages: (SidebarNavigationPage | "separator")[] | { title: CompatibilityToolFlavor; content: JSX.Element; route: string; }[] = []
    appState.available_flavors.forEach((flavor) => {
      pages.push({
        title: flavor.flavor,
        content: (
          <FlavorTab
            getAppState={appState}
            getFlavor={flavor}
            getSocket={socket}
          />
        ),
        route: "/wine-cellar/" + flavor.flavor,
      });
    });
    //return <SidebarNavigation title="Wine Cellar" showTitle pages={flavor_pages}/>;
  }

  pages.push({
    title: "Virtual",
    content: <VirtualCompatibilityTools />,
    route: "/wine-cellar/virtual",
  });

  return <SidebarNavigation title="Wine Cellar" showTitle pages={pages} />;
}
