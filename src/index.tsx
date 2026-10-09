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
import { startLocalization, useTranslation } from "./i18n";

import ManagePage from "./frontend";
import { forceCloseToastsWebSocket, setupToasts } from "./utils/toasts";
import { GiCellarBarrels } from "react-icons/gi";

const Content: FC = () => {
  const { t } = useTranslation();
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
            {t("common-manage")}
        </ButtonItem>
      </PanelSectionRow>
    </PanelSection>
  );
};

export default definePlugin(() => {
  void startLocalization();
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
