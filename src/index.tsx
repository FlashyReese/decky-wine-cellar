import {
  ButtonItem,
  definePlugin,
  Navigation,
  PanelSection,
  PanelSectionRow,
  Router,
  ServerAPI,
  staticClasses,
} from "decky-frontend-lib";
import { VFC } from "react";
import { FaShip } from "react-icons/fa";

import ManagePage from "./frontend";
import { Request, RequestType } from "./types";
import { log } from "./logger";
import {RegisterForShutdownStart} from "./SteamUtil";

const Content: VFC<{ serverAPI: ServerAPI }> = ({}) => {
  return (
    <PanelSection title="Wine Cellar">
      <PanelSectionRow>
        <ButtonItem
          layout="below"
          onClick={() => {
            Router.CloseSideMenus();
            Router.Navigate("/wine-cellar");
          }}
        >
          Manage
        </ButtonItem>
        <ButtonItem
          layout="below"
          onClick={() => {
            Navigation.CloseSideMenus();
            Navigation.NavigateToExternalWeb(
              "https://github.com/FlashyReese/decky-wine-cellar/issues",
            );
          }}
        >
          Report an issue
        </ButtonItem>
        <ButtonItem
          layout="below"
          onClick={() => {
            Navigation.CloseSideMenus();
            Navigation.NavigateToExternalWeb("https://ko-fi.com/flashyreese");
          }}
        >
          Support the project!
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
};

export default definePlugin((serverApi: ServerAPI) => {
  serverApi.routerHook.addRoute("/wine-cellar", () => {
    return <ManagePage />;
  });

  let shutdownHook = RegisterForShutdownStart(() => {
    log("We are attempting to restart the backend, hold on :P");
    const ws = new WebSocket("ws://localhost:8887");
    ws.onopen = (): void => {
      const response: Request = {
        type: RequestType.Reboot,
      };
      ws.send(JSON.stringify(response));
      ws.close();
      shutdownHook!.unregister();
    };
  });

  return {
    title: <div className={staticClasses.Title}>Wine Cellar</div>,
    content: <Content serverAPI={serverApi} />,
    icon: <FaShip />,
    onDismount() {
      serverApi.routerHook.removeRoute("/wine-cellar");
    },
  };
});
