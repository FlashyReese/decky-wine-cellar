import { call } from "@decky/api";
import { debug } from "./logger";

export class BackendCtx {
  static async bridge(functionName: string, ...args: unknown[]) {
    debug(`Calling backend function: ${functionName}`);
    let output = await call(functionName, ...args);
    return output;
  }

  static async getSetting(key: string, defaults: any) {
    return await this.bridge("settings_getSetting", key, defaults);
  }

  static async setSetting(key: string, value: any) {
    return await this.bridge("settings_setSetting", key, value);
  }

  static async commitSettings() {
    return await this.bridge("settings_commit");
  }
}
