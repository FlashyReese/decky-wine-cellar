import { call } from "@decky/api";
import { debug } from "./logger";

export class BackendCtx {
  static async bridge(functionName: string, ...args: unknown[]) {
    debug(`Calling backend function: ${functionName}`);
    const output = await call(functionName, ...args);
    return output;
  }

  static async getSetting(key: string, defaults: unknown) {
    return await this.bridge("settings_getSetting", key, defaults);
  }

  static async setSetting(key: string, value: unknown) {
    return await this.bridge("settings_setSetting", key, value);
  }

  static async commitSettings() {
    return await this.bridge("settings_commit");
  }
}
