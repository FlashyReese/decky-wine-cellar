import {
  routerHook,
} from "@decky/api";
import {
  ButtonItem,
  definePlugin,
  PanelSection,
  PanelSectionRow,
  Router,
  staticClasses,
} from "@decky/ui";
import { FC } from "react";

import ManagePage from "./frontend";
import { forceCloseToastsWebSocket, setupToasts } from "./utils/toasts";
import { GiCellarBarrels } from "react-icons/gi";

const Content: FC = () => {
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
