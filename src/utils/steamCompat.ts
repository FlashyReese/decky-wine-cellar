import { findModule, findModuleExport } from "@decky/ui";
import type { CompatToolInfo } from "./steamUtils";

interface BinaryReader {
  nextField(): boolean;
  isEndGroup(): boolean;
  getFieldNumber(): number;
  readString(): string;
  readBool(): boolean;
  readMessage<T>(
    value: T,
    read: (value: T, reader: BinaryReader) => void,
  ): void;
  skipField(): void;
}

interface BinaryWriter {
  writeUint32(field: number, value: number): void;
  getResultBuffer(): Uint8Array;
}

interface CompatTool extends CompatToolInfo {
  active?: boolean;
  legacy?: boolean;
  incompatible?: boolean;
}

// Steam's CompatManager service replaced the native GetGlobalCompatTools and
// GetAvailableCompatTools methods. Use Steam's existing authenticated transport
// and protobuf framing; only decode the fields needed for the tool menus.
class CompatToolsResponse {
  tools: CompatTool[] = [];

  static deserializeBinaryFromReader(
    response: CompatToolsResponse,
    reader: BinaryReader,
  ): void {
    while (reader.nextField() && !reader.isEndGroup()) {
      const field = reader.getFieldNumber();
      if (field !== 1 && field !== 2) {
        reader.skipField();
        continue;
      }

      const tool: CompatTool = { strToolName: "", strDisplayName: "" };
      const isAlias = field === 2;
      reader.readMessage(tool, (item, fields) => {
        while (fields.nextField() && !fields.isEndGroup()) {
          switch (fields.getFieldNumber()) {
            case 1:
              item.strToolName = fields.readString();
              break;
            case 2:
              item.strDisplayName = fields.readString();
              break;
            case 4:
            case 5:
            case 6:
              if (!isAlias) {
                const key = { 4: "active", 5: "legacy", 6: "incompatible" }[
                  fields.getFieldNumber()
                ] as "active" | "legacy" | "incompatible";
                item[key] = fields.readBool();
              } else {
                fields.skipField();
              }
              break;
            default:
              fields.skipField();
          }
        }
      });
      if (tool.strToolName) response.tools.push(tool);
    }
  }
}

interface SteamResponse {
  BIsValid(): boolean;
  BSuccess(): boolean;
  GetErrorMessage(): string;
  Body(): CompatToolsResponse;
}

interface CompatTransport {
  SendMsg(
    method: string,
    request: unknown,
    response: typeof CompatToolsResponse,
    options: { ePrivilege: number; eClientExecutionSite: number },
  ): Promise<SteamResponse>;
}

interface ProtoMessage {
  Init(body: new () => { serializeBinary(): Uint8Array }): unknown;
}

export async function getCompatManagerTools(
  appId?: number,
): Promise<CompatToolInfo[]> {
  const transport = findModuleExport(
    (value) => value?.SendMsg && value?.GetConnectionDetails,
  ) as CompatTransport | undefined;
  const message = findModuleExport(
    (value) => value?.Init && value?.InitFromMsg && value?.InitFromPacket,
  ) as ProtoMessage | undefined;
  const protobuf = findModule(
    (value) => value?.BinaryWriter && value?.BinaryReader,
  ) as { BinaryWriter: new () => BinaryWriter } | undefined;

  if (!transport || !message || !protobuf) {
    throw new Error("Steam compatibility tool service is unavailable");
  }

  const Writer = protobuf.BinaryWriter;
  class CompatToolsRequest {
    serializeBinary(): Uint8Array {
      const writer = new Writer();
      if (appId != null) writer.writeUint32(1, appId);
      return writer.getResultBuffer();
    }
  }

  const response = await transport.SendMsg(
    "CompatManager.GetCompatTools#1",
    message.Init(CompatToolsRequest),
    CompatToolsResponse,
    { ePrivilege: 1, eClientExecutionSite: 2 },
  );
  if (!response.BIsValid() || !response.BSuccess()) {
    throw new Error(response.GetErrorMessage());
  }

  return response
    .Body()
    .tools.filter(
      (tool) =>
        appId == null || tool.active || (!tool.legacy && !tool.incompatible),
    )
    .map(({ strToolName, strDisplayName }) => ({
      strToolName,
      strDisplayName: strDisplayName || strToolName,
    }));
}
