import {
  Command,
  CommandType,
  MessageEnvelope,
  MessageType,
} from "../types";
import { GetGlobalCompatTools } from "./steamUtils";
import { error } from "./logger";

function sendMessage(socket: WebSocket, message: MessageEnvelope): void {
  if (socket.readyState !== WebSocket.OPEN) {
    error("WebSocket not alive...");
    return;
  }

  socket.send(JSON.stringify(message));
}

function sendCommand(socket: WebSocket, command: Command): void {
  sendMessage(socket, {
    type: MessageType.Command,
    command,
  });
}

export function requestState(socket: WebSocket): void {
  sendMessage(socket, {
    type: MessageType.GetState,
  });
}

export async function reportSteamVisibleTools(socket: WebSocket): Promise<void> {
  const tools = await GetGlobalCompatTools();
  sendMessage(socket, {
    type: MessageType.ReportSteamVisibleTools,
    steam_visible_tools: tools,
  });
}

export function refreshCatalog(socket: WebSocket): void {
  sendCommand(socket, {
    type: CommandType.RefreshCatalog,
  });
}

export function installCatalogRelease(socket: WebSocket, releaseId: string): void {
  sendCommand(socket, {
    type: CommandType.InstallCatalogRelease,
    release_id: releaseId,
  });
}

export function uninstallInstalledTool(
  socket: WebSocket,
  installedToolId: string,
): void {
  sendCommand(socket, {
    type: CommandType.UninstallInstalledTool,
    installed_tool_id: installedToolId,
  });
}

export function cancelOperation(socket: WebSocket, operationId: string): void {
  sendCommand(socket, {
    type: CommandType.CancelOperation,
    operation_id: operationId,
  });
}
