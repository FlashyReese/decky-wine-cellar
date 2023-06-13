import {AppState, Request, RequestType} from "./types";
import {log} from "./logger";

class WebSocketBridge {
    private static instance: WebSocketBridge | null = null;
    private readonly webSocket: WebSocket;
    private appState: AppState | null = null

    private constructor() {
        this.webSocket = new WebSocket("ws://localhost:8887");
        this.webSocket.onopen = () => {
            log('WebSocket connection established.');
            const response: Request = {
                type: RequestType.RequestState
            };
            this.webSocket.send(JSON.stringify(response));
        };
        this.webSocket.onmessage = (event) => {
            const response: Request = JSON.parse(event.data);
            if (response.type == RequestType.UpdateState) {
                if (response.app_state != null) {
                    this.appState = response.app_state;
                }
            }
        }
        this.webSocket.onclose = () => {

        }
    }

    static getInstance(): WebSocketBridge {
        if (!WebSocketBridge.instance) {
            WebSocketBridge.instance = new WebSocketBridge();
        }
        return WebSocketBridge.instance;
    }

    getWebSocket(): WebSocket {
        return this.webSocket;
    }

    getAppState(): AppState | null {
        return this.appState;
    }
}

export default WebSocketBridge;