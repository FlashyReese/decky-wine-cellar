import { Dropdown, Field, Focusable, SingleDropdownOption } from "@decky/ui";
import { useEffect, useRef, useState } from "react";
import { DisplayMessage, useTranslation } from "../i18n";
import { CompatToolInfo, ManagedSteamApplication } from "../utils/steamUtils";

export type ToolOptionsState =
  | { status: "loading" }
  | { status: "ready"; tools: CompatToolInfo[] }
  | { status: "error"; message: DisplayMessage };

interface ApplicationRowProps {
  application: ManagedSteamApplication;
  assignedToolName: string;
  assignedToolDisplayName: string;
  optionsState?: ToolOptionsState;
  isSaving: boolean;
  assignmentError?: DisplayMessage;
  disabled: boolean;
  onMenuWillOpen: (showMenu: () => void, isFocused: () => boolean) => void;
  onChange: (option: SingleDropdownOption) => void;
}

function dropdownOptions(
  tools: CompatToolInfo[],
  assignedToolName: string,
  assignedToolDisplayName: string,
  steamDefaultLabel: string,
): SingleDropdownOption[] {
  const seenToolNames = new Set<string>();
  const options = tools.flatMap((tool) => {
    if (tool.strToolName === "" || seenToolNames.has(tool.strToolName)) {
      return [];
    }

    seenToolNames.add(tool.strToolName);
    return [
      {
        data: tool.strToolName,
        label: tool.strDisplayName || tool.strToolName,
      },
    ];
  });

  if (assignedToolName !== "" && !seenToolNames.has(assignedToolName)) {
    options.push({ data: assignedToolName, label: assignedToolDisplayName });
  }

  return [{ data: "", label: steamDefaultLabel }, ...options];
}

function ApplicationIcon({
  application,
}: {
  application: ManagedSteamApplication;
}) {
  const [imageFailed, setImageFailed] = useState(false);

  useEffect(() => {
    setImageFailed(false);
  }, [application.iconUrl]);

  const showImage = application.iconUrl != null && !imageFailed;
  const fallback = application.name.trim().charAt(0).toLocaleUpperCase() || "?";

  return (
    <div
      aria-hidden="true"
      style={{
        width: "32px",
        minWidth: "32px",
        height: "32px",
        boxSizing: "border-box",
        overflow: "hidden",
        display: "grid",
        placeItems: "center",
        border: "1px solid rgba(255, 255, 255, 0.12)",
        borderRadius: "4px",
        background: "rgba(255, 255, 255, 0.08)",
        fontSize: "18px",
        fontWeight: 600,
      }}
    >
      {showImage ? (
        <img
          src={application.iconUrl}
          alt=""
          loading="lazy"
          decoding="async"
          onError={() => setImageFailed(true)}
          style={{
            display: "block",
            width: "32px",
            height: "32px",
            objectFit: "cover",
          }}
        />
      ) : (
        <span>{fallback}</span>
      )}
    </div>
  );
}

export default function ApplicationRow({
  application,
  assignedToolName,
  assignedToolDisplayName,
  optionsState,
  isSaving,
  assignmentError,
  disabled,
  onMenuWillOpen,
  onChange,
}: ApplicationRowProps) {
  const { t, translateMessage } = useTranslation();
  const rowRef = useRef<HTMLLIElement>(null);
  const availableTools =
    optionsState?.status === "ready" ? optionsState.tools : [];
  const hasDescription =
    optionsState?.status === "loading" ||
    optionsState?.status === "error" ||
    isSaving ||
    assignmentError != null;

  return (
    <li ref={rowRef} style={{ margin: 0, padding: 0 }}>
      <Field
        label={
          <div style={{ minWidth: 0 }}>
            <div
              title={t("applications-appTitle", {
                name: application.name,
                id: String(application.appId),
              })}
              style={{
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
            >
              {application.name}
            </div>
            <div style={{ fontSize: "12px", lineHeight: "16px", opacity: 0.65 }}>
              {application.isShortcut
                ? t("applications-shortcut")
                : t("applications-steamGame")}
            </div>
          </div>
        }
        description={
          hasDescription && (
            <div>
              {optionsState?.status === "loading" && (
                <div role="status">{t("applications-loadingTools")}</div>
              )}
              {optionsState?.status === "error" && (
                <div role="alert" style={{ color: "#ff9f9f" }}>
                  {t("applications-toolsFailed", { error: optionsState.message })}
                </div>
              )}
              {isSaving && (
                <div role="status">{t("applications-saving")}</div>
              )}
              {assignmentError != null && (
                <div role="alert" style={{ color: "#ff9f9f" }}>
                  {translateMessage(assignmentError)}
                </div>
              )}
            </div>
          )
        }
        icon={<ApplicationIcon application={application} />}
        bottomSeparator="standard"
        inlineWrap="keep-inline"
        childrenContainerWidth="min"
        verticalAlignment="center"
        padding="compact"
      >
        <Focusable
          flow-children="row"
          style={{
            width: "clamp(160px, 23vw, 260px)",
            maxWidth: "100%",
            minWidth: 0,
            boxShadow: "none",
          }}
        >
          <Dropdown
            rgOptions={dropdownOptions(
              availableTools,
              assignedToolName,
              assignedToolDisplayName,
              t("applications-steamDefault"),
            )}
            selectedOption={assignedToolName}
            disabled={disabled}
            menuLabel={t("applications-toolFor", { name: application.name })}
            focusable
            renderButtonValue={() => (
              <span
                aria-label={t("applications-selectedTool", {
                  name: application.name,
                  tool: assignedToolDisplayName,
                })}
                title={assignedToolDisplayName}
                style={{
                  display: "block",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {assignedToolDisplayName}
              </span>
            )}
            onMenuWillOpen={(showMenu) =>
              onMenuWillOpen(showMenu, () => {
                const row = rowRef.current;
                return row != null && row.isConnected &&
                  row.contains(row.ownerDocument.activeElement);
              })
            }
            onChange={onChange}
          />
        </Focusable>
      </Field>
    </li>
  );
}
