import { ServerAPI } from "decky-frontend-lib";
import { debug } from "./logger";

export class BackendCtx {
  static initialize(serverApi: ServerAPI) {
    this.serverAPI = serverApi;
  }

  static serverAPI: ServerAPI;

  static async bridge(functionName: string, namedArgs?: any) {
    namedArgs = namedArgs ? namedArgs : {};
    debug(`Calling backend function: ${functionName}`);
    let output = await this.serverAPI.callPluginMethod(functionName, namedArgs);
    return output.result;
  }

  static async getSetting(key: string, defaults: any) {
    return await this.bridge("settings_getSetting", { key, defaults });
  }

  static async setSetting(key: string, value: any) {
    return await this.bridge("settings_setSetting", { key, value });
  }

  static async commitSettings() {
    return await this.bridge("settings_commit");
  }

  static async restartBackend() {
    return await this.bridge("restart_backend");
  }
}
