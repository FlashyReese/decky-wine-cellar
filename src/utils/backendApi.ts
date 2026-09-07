import {
  Command,
  CommandType,
  InstallTargetType,
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
    target: {
      type: InstallTargetType.Direct,
    },
  });
}

export function mountCatalogReleaseToVirtualTool(
  socket: WebSocket,
  releaseId: string,
  virtualToolId: string,
): void {
  sendCommand(socket, {
    type: CommandType.InstallCatalogRelease,
    release_id: releaseId,
    target: {
      type: InstallTargetType.VirtualTool,
      virtual_tool_id: virtualToolId,
    },
  });
}

export function linkInstalledToolToVirtualTool(
  socket: WebSocket,
  installedToolId: string,
  virtualToolId: string,
): void {
  sendCommand(socket, {
    type: CommandType.LinkInstalledToolToVirtualTool,
    installed_tool_id: installedToolId,
    virtual_tool_id: virtualToolId,
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

export function createVirtualTool(socket: WebSocket, userLabel: string): void {
  sendCommand(socket, {
    type: CommandType.CreateVirtualTool,
    user_label: userLabel,
  });
}

export function renameVirtualTool(
  socket: WebSocket,
  virtualToolId: string,
  userLabel: string,
): void {
  sendCommand(socket, {
    type: CommandType.RenameVirtualTool,
    virtual_tool_id: virtualToolId,
    user_label: userLabel,
  });
}

export function removeVirtualTool(
  socket: WebSocket,
  virtualToolId: string,
): void {
  sendCommand(socket, {
    type: CommandType.RemoveVirtualTool,
    virtual_tool_id: virtualToolId,
  });
}
