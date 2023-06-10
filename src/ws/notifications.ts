import {ServerAPI, ToastData} from 'decky-frontend-lib';
import {error, log} from '../logger';
import {Response, ResponseType} from "../types";


let ws: WebSocket | null = null;
let mounted: boolean = false;
export const setupNotifications = (serverAPI: ServerAPI): void => {
    const handleMessage = (message: String): void => {
        let toastData: ToastData = {
            title: "Wine Cellar",
            body: message,
            showToast: true
        }

        serverAPI.toaster.toast(toastData);
    }

    const setupWebsocket = (): void => {
        ws = new WebSocket('ws://localhost:8887');

        ws.onopen = (): void => {
            log('Notification WebSocket connected');
        };

        ws.onmessage = (e: MessageEvent): void => {
            const response: Response = JSON.parse(e.data);
            if (response.type == ResponseType.Notification && response.message != null) {
                handleMessage(response.message);
            }
        };

        ws.onclose = (e: CloseEvent): void => {
            log('Notification WebSocket is closed. Reconnect will be attempted in 10 seconds.', e.reason);
            setTimeout(() => {
                if (mounted) {
                    setupWebsocket();
                }
            }, 10000);
        };

        ws.onerror = (err: Event): void => {
            error('Notification WebSocket encountered error: ', (err as ErrorEvent).message, 'Closing socket');
            if (ws != null) {
                ws.close();
            }
        };
    }

    setupWebsocket();
    mounted = true;
}

export const unmountNotifications = (): void => {
    mounted = false;
    log("Closing notifications websocket...")
    ws?.close();
    log("Closed notifications websocket!")
}