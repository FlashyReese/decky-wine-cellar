import {
  ButtonItem,
  definePlugin,
  PanelSection,
  PanelSectionRow,
  Router,
  ServerAPI,
  staticClasses,
} from "decky-frontend-lib";
import { VFC } from "react";
import { FaShip } from "react-icons/fa";

import ManagePage from "./frontend";
import { forceCloseToastsWebSocket, setupToasts } from "./utils/toasts";

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
      </PanelSectionRow>
    </PanelSection>
  );
};

export default definePlugin((serverApi: ServerAPI) => {
  setupToasts(serverApi);
  serverApi.routerHook.addRoute("/wine-cellar", () => {
    return <ManagePage />;
  });

  return {
    title: <div className={staticClasses.Title}>Wine Cellar</div>,
    content: <Content serverAPI={serverApi} />,
    icon: <FaShip />,
    onDismount() {
      forceCloseToastsWebSocket();
      serverApi.routerHook.removeRoute("/wine-cellar");
    },
  };
});
