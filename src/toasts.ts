import { ServerAPI, ToastData } from 'decky-frontend-lib';
import { log, error } from './logger';
import {Request, RequestType} from "./types";

export const setupToasts = (serverAPI: ServerAPI): void => {
    const handleMessage = (e: MessageEvent): void => {
        const response: Request = JSON.parse(e.data);
        if (response.type == RequestType.Notification && response.notification != null) {
            let toastData: ToastData = {
                title: "Wine Cellar",
                body: response.notification,
                showToast: true
            }
            serverAPI.toaster.toast(toastData);
        }
    }

    const setupWebsocket = (): void => {
        const ws = new WebSocket('ws://localhost:8887');

        ws.onopen = (): void => {
            log('WebSocket connected');
        };

        ws.onmessage = (e: MessageEvent): void => {
            handleMessage(e);
        };

        ws.onclose = (e: CloseEvent): void => {
            log('Socket is closed. Reconnect will be attempted in 10 seconds.', e.reason);
            setTimeout(() => {
                setupWebsocket();
            }, 10000);
        };

        ws.onerror = (err: Event): void => {
            error('Socket encountered error: ', (err as ErrorEvent).message, 'Closing socket');
            ws.close();
        };
    }

    setupWebsocket();
}