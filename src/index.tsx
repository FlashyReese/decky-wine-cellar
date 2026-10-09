import { routerHook } from "@decky/api";
import {
  ButtonItem,
  definePlugin,
  PanelSection,
  PanelSectionRow,
  Router,
  staticClasses,
} from "@decky/ui";
import { FC, useEffect } from "react";

import ManagePage from "./frontend";
import { forceCloseToastsWebSocket, setupToasts } from "./utils/toasts";
import { GiCellarBarrels } from "react-icons/gi";

const Content: FC = () => {
  const openManage = () => {
    Router.Navigate("/wine-cellar");
    Router.CloseSideMenus();
  };

  useEffect(() => {
    const deckyState = (window as Window & {
      DeckyPluginLoader?: {
        deckyState?: { closeActivePlugin?: () => void };
      };
    }).DeckyPluginLoader?.deckyState;
    if (typeof deckyState?.closeActivePlugin !== "function") return;

    deckyState.closeActivePlugin();
    openManage();
  }, []);

  return (
    <PanelSection title="Wine Cellar">
      <PanelSectionRow>
        <ButtonItem layout="below" onClick={openManage}>
          Manage
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
};

export default definePlugin(() => {
  setupToasts();
  routerHook.addRoute("/wine-cellar", () => {
    return <ManagePage />;
  });

  return {
    title: <div className={staticClasses.Title}>Wine Cellar</div>,
    content: <Content />,
    icon: <GiCellarBarrels />,
    onDismount() {
      forceCloseToastsWebSocket();
      routerHook.removeRoute("/wine-cellar");
    },
  };
});
