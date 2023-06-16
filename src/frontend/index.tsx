import {SidebarNavigation, SidebarNavigationPage} from 'decky-frontend-lib';

import {useEffect, useState} from "react";
import {AppState, Request, RequestType} from "../types";
import {error, log} from "../logger";
import FlavorTab from "./CompatibilityToolFlavorTab";
import VirtualCompatibilityTools from "./VirtualCompatibilityTools";

export default function ManagePage() {
    const [appState, setAppState] = useState<AppState | null>(null);

    const [socket, setSocket] = useState<WebSocket>();

    useEffect(() => {

        const socket = new WebSocket("ws://localhost:8887");
        setSocket(socket);

        socket.onopen = async () => {
            log('WebSocket connection established.');
            const response: Request = {
                type: RequestType.RequestState
            };
            socket.send(JSON.stringify(response));
        };

        socket.onmessage = async (event) => {
            const response: Request = JSON.parse(event.data);
            if (response.type == RequestType.UpdateState) {
                if (response.app_state != null) {
                    setAppState(response.app_state);
                }
            }
        };

        socket.onclose = () => {
            log('WebSocket connection closed.');
        };

        return () => {
            socket.close(); // Close the WebSocket connection on component unmount
        };
    }, [])


    // todo: We should move releases list to the backend we should be able to construct our pages easily and this way we can atleast list something if our connection is offline
    const pages: (SidebarNavigationPage | "separator")[] = [
        {
            title: "Virtual",
            content: <VirtualCompatibilityTools/>,
            route: '/wine-cellar/virtual'
        }
    ];

    if (appState != null && socket != null) {
        //const flavor_pages: (SidebarNavigationPage | "separator")[] | { title: CompatibilityToolFlavor; content: JSX.Element; route: string; }[] = []
        appState.available_flavors.forEach(flavor => {
            pages.push({
                title: flavor.flavor,
                content: <FlavorTab getAppState={appState} getFlavor={flavor} getSocket={socket}/>,
                route: '/wine-cellar/' + flavor.flavor
            });
        })
        //return <SidebarNavigation title="Wine Cellar" showTitle pages={flavor_pages}/>;
    }


    return <SidebarNavigation title="Wine Cellar" showTitle pages={pages}/>;
}