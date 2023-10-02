import { SidebarNavigation, SidebarNavigationPage } from "decky-frontend-lib";

import { useEffect, useState } from "react";
import { AppState, Request, RequestType } from "../types";
import { log } from "../utils/logger";
import { v4 as uuidv4 } from "uuid";
import FlavorTab from "./flavorTab";
import ManagerTab from "./manager";
import { GetGlobalCompatTools } from "../utils/steamUtils";
import About from "./about";
import GamesTab from "./games";


export default function ManagePage() {
  const [appState, setAppState] = useState<AppState | undefined>();

  const [socket, setSocket] = useState<WebSocket>();

  useEffect(() => {
    const socket = new WebSocket("ws://localhost:8887");
    const uniqueId = uuidv4(); // Generate a unique identifier

    setSocket(socket);

    socket.onopen = async () => {
      log("WebSocket connection established. Unique Identifier:", uniqueId); // Log the unique identifier on connection open

      const tools = await GetGlobalCompatTools();

      const response: Request = {
        type: RequestType.RequestState,
        available_compat_tools: tools,
      };

      socket.send(JSON.stringify(response));
    };

    socket.onmessage = async (event) => {
      //log("Received message from server:", event.data);
      const response: Request = JSON.parse(event.data);
      if (response.type == RequestType.UpdateState) {
        if (response.app_state != null) {
          setAppState(response.app_state);
          log("Received app state update");
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
    // Regular dashboard
    pages.push({
      title: "Dashboard",
      content: <ManagerTab appState={appState} socket={socket} />,
      route: "/wine-cellar/dashboard",
    });

    pages.push({
      title: "Games",
      content: <GamesTab appState={appState} socket={socket} />,
      route: "/wine-cellar/games",
    });

    // Flavor pages
    appState.available_flavors.forEach((flavor) => {
      pages.push({
        title: flavor.flavor,
        content: (
          <FlavorTab appState={appState} flavor={flavor} socket={socket} />
        ),
        route: "/wine-cellar/" + flavor.flavor,
      });
    });
  } else {
    // Loading page
    pages.push({
      title: "Preparing...",
      content: (
        <div>
          Hang tight! We're preparing your Wine Cellar experience. If this is
          taking longer than expected, the backend might be having a siesta.
        </div>
      ),
      route: "/wine-cellar/preparing",
    });
  }

  // About page
  pages.push({
    title: "About",
    content: <About appState={appState} socket={socket} />,
    route: "/wine-cellar/about",
  });

  return <SidebarNavigation title="Wine Cellar" showTitle pages={pages} />;
}
