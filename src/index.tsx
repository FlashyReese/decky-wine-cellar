import {
    ButtonItem,
    definePlugin,
    PanelSection,
    PanelSectionRow, Router,
    ServerAPI,
    staticClasses,
} from "decky-frontend-lib";
import {VFC} from "react";
import {FaShip} from "react-icons/fa";

import ManagePage from "./frontend";
import {Request, RequestType} from "./types";
import {log} from "./logger";
import {setupToasts} from "./toasts";

const Content: VFC<{ serverAPI: ServerAPI }> = ({}) => {
    return (
        <PanelSection title="Wine Cellar">
            <PanelSectionRow>
                <ButtonItem
                    layout="below"
                    onClick={() => {
                        Router.CloseSideMenus();
                        Router.Navigate("/wine-cellar");
                    }
                    }
                >
                    Manage
                </ButtonItem>
            </PanelSectionRow>
        </PanelSection>
    );
};

export default definePlugin((serverApi: ServerAPI) => {
    setupToasts(serverApi);
    serverApi.routerHook.addRoute('/wine-cellar', () => {
        return (
            <ManagePage/>
        );
    });

    SteamClient.User.RegisterForShutdownStart(() => {
        log("We are attempting to restart the backend, hold on :P");
        // Fixme: If task is being performed at the backend this will stall and probably not run?
        const ws = new WebSocket("ws://localhost:8887");
        ws.onopen = (): void => {
            const response: Request = {
                type: RequestType.Reboot,
            };
            ws.send(JSON.stringify(response));
            ws.close();
        }
    })

    return {
        title: <div className={staticClasses.Title}>Wine Cellar</div>,
        content: <Content serverAPI={serverApi}/>,
        icon: <FaShip/>,
        onDismount() {
            //unmountNotifications();
            serverApi.routerHook.removeRoute("/wine-cellar");
        },
    };
});
