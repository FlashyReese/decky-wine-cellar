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

import ManagePage from "./frontend";
import { forceCloseToastsWebSocket, setupToasts } from "./utils/toasts";
import { GiCellarBarrels } from "react-icons/gi";
import {BackendCtx} from "./utils/pythonBackendHelper";

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

  BackendCtx.initialize(serverApi);

  return {
    title: <div className={staticClasses.Title}>Wine Cellar</div>,
    content: <Content serverAPI={serverApi} />,
    icon: <GiCellarBarrels />,
    onDismount() {
      forceCloseToastsWebSocket();
      serverApi.routerHook.removeRoute("/wine-cellar");
    },
  };
});
