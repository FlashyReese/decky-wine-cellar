import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  Dropdown,
  Focusable,
  GamepadButton,
  GamepadEvent,
  SingleDropdownOption,
  TextField,
} from "@decky/ui";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { AppState } from "../types";
import ApplicationRow, { ToolOptionsState } from "./applicationRow";
import { requestState } from "../utils/backendApi";
import {
  DisplayMessage,
  LocalizedMessage,
  errorMessage,
  message,
  useTranslation,
} from "../i18n";
import {
  GetAvailableCompatTools,
  GetManagedApplications,
  ManagedSteamApplication,
  SpecifyCompatTool,
} from "../utils/steamUtils";

const APPLICATIONS_PER_PAGE = 25;

type ApplicationSource = "steam" | "shortcuts" | "all";

interface ApplicationsTabProps {
  appState: AppState;
  socket: WebSocket;
}

export default function ApplicationsTab({
  appState,
  socket,
}: ApplicationsTabProps) {
  const { t, translateMessage } = useTranslation();
  const [applications, setApplications] = useState<ManagedSteamApplication[]>(
    [],
  );
  const [search, setSearch] = useState("");
  const [source, setSource] = useState<ApplicationSource>("steam");
  const searchFieldRef = useRef<HTMLDivElement>(null);
  const applicationListRef = useRef<HTMLUListElement>(null);
  const restoreListFocus = useRef(false);
  const [page, setPage] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<DisplayMessage>();
  const [inventoryWarning, setInventoryWarning] = useState<LocalizedMessage>();
  const [assignmentErrors, setAssignmentErrors] = useState<
    Record<number, DisplayMessage>
  >({});
  const [pendingAssignments, setPendingAssignments] = useState<
    Record<number, string>
  >({});
  const [toolOptions, setToolOptions] = useState<
    Record<number, ToolOptionsState>
  >({});
  const loadGeneration = useRef(0);
  const toolCacheGeneration = useRef(0);

  const compatibilityMappings = useMemo(
    () => appState.app_compat_tool_mappings ?? {},
    [appState.app_compat_tool_mappings],
  );
  const compatibilityMappingsStale =
    appState.app_compat_tool_mappings_stale ?? true;
  const toolDisplayNames = useMemo(() => {
    const names = new Map<string, string>();
    (appState.steam_visible_tools ?? []).forEach((tool) => {
      names.set(tool.strToolName, tool.strDisplayName || tool.strToolName);
    });
    appState.installed_tools.forEach((tool) => {
      if (!names.has(tool.internal_name)) {
        names.set(tool.internal_name, tool.display_name || tool.internal_name);
      }
    });
    return names;
  }, [appState.installed_tools, appState.steam_visible_tools]);

  const loadApplications = useCallback(async () => {
    const generation = ++loadGeneration.current;
    setIsLoading(true);
    setLoadError(undefined);

    try {
      const nextInventory = await GetManagedApplications();

      if (generation !== loadGeneration.current) {
        return;
      }

      setApplications(nextInventory.applications);
      setInventoryWarning(nextInventory.warning);
    } catch (error) {
      if (generation === loadGeneration.current) {
        setLoadError(errorMessage(error));
      }
    } finally {
      if (generation === loadGeneration.current) {
        setIsLoading(false);
      }
    }
  }, []);

  const resetToolOptions = useCallback(() => {
    toolCacheGeneration.current += 1;
    setToolOptions({});
  }, []);

  const requestLatestState = useCallback(() => {
    try {
      requestState(socket);
    } catch {
      // Inventory remains useful while the management socket reconnects.
    }
  }, [socket]);

  const refresh = useCallback(() => {
    resetToolOptions();
    requestLatestState();
    void loadApplications();
    setPendingAssignments({});
  }, [loadApplications, requestLatestState, resetToolOptions]);

  useEffect(() => {
    refresh();

    return () => {
      loadGeneration.current += 1;
      toolCacheGeneration.current += 1;
    };
  }, [refresh]);

  // Confirm only from fresh snapshots; Steam can flush its config after the setter returns.
  useEffect(() => {
    if (compatibilityMappingsStale) {
      return;
    }
    setPendingAssignments((current) => {
      const remaining = Object.entries(current).filter(
        ([appId, toolName]) =>
          (compatibilityMappings[appId] ?? "") !== toolName,
      );
      return remaining.length === Object.keys(current).length
        ? current
        : Object.fromEntries(remaining);
    });
  }, [compatibilityMappings, compatibilityMappingsStale]);

  const assignmentFor = useCallback(
    (appId: number): string =>
      pendingAssignments[appId] ?? compatibilityMappings[appId] ?? "",
    [compatibilityMappings, pendingAssignments],
  );

  const toolDisplayNameFor = useCallback(
    (toolName: string): string =>
      toolName === ""
        ? t("applications-steamDefault")
        : toolDisplayNames.get(toolName) || toolName,
    [toolDisplayNames, t],
  );

  const sourceApplications = useMemo(
    () =>
      applications.filter(
        (application) =>
          source === "all" ||
          (source === "steam"
            ? !application.isShortcut
            : application.isShortcut),
      ),
    [applications, source],
  );

  const filteredApplications = useMemo(() => {
    const normalizedSearch = search.trim().toLocaleLowerCase();
    if (normalizedSearch === "") {
      return sourceApplications;
    }

    return sourceApplications.filter((application) => {
      const assignedToolName = assignmentFor(application.appId);
      const assignedToolDisplayName = toolDisplayNameFor(assignedToolName);

      return [
        application.name,
        application.sortAs,
        String(application.appId),
        assignedToolName,
        assignedToolDisplayName,
        application.isShortcut
          ? t("applications-shortcut")
          : t("applications-steamGame"),
      ].some((value) => value.toLocaleLowerCase().includes(normalizedSearch));
    });
  }, [sourceApplications, assignmentFor, search, toolDisplayNameFor, t]);

  const pageCount = Math.max(
    1,
    Math.ceil(filteredApplications.length / APPLICATIONS_PER_PAGE),
  );
  const canGoToPreviousPage = page > 0;
  const canGoToNextPage = page + 1 < pageCount;

  const changePage = (offset: number) => {
    const list = applicationListRef.current;
    restoreListFocus.current =
      list?.contains(list.ownerDocument.activeElement) ?? false;
    setPage((current) =>
      Math.max(0, Math.min(pageCount - 1, current + offset)),
    );
  };
  const previousPage = () => changePage(-1);
  const nextPage = () => changePage(1);

  useLayoutEffect(() => {
    if (restoreListFocus.current) {
      restoreListFocus.current = false;
      applicationListRef.current
        ?.querySelector<HTMLButtonElement>('button[role="combobox"]')
        ?.focus();
    }
  }, [page]);

  const focusSearch = (event: GamepadEvent) => {
    event.stopPropagation();
    if (event.detail.is_repeat) {
      return;
    }

    const input = searchFieldRef.current?.querySelector("input");
    input?.focus();
    input?.click();
  };

  const handlePageButton = (event: GamepadEvent) => {
    const button = event.detail.button;
    if (
      button !== GamepadButton.BUMPER_LEFT &&
      button !== GamepadButton.BUMPER_RIGHT
    ) {
      return;
    }

    event.stopPropagation();
    if (event.detail.is_repeat) {
      return;
    }

    if (button === GamepadButton.BUMPER_LEFT && canGoToPreviousPage) {
      previousPage();
    } else if (button === GamepadButton.BUMPER_RIGHT && canGoToNextPage) {
      nextPage();
    }
  };

  useEffect(() => {
    setPage((current) => Math.min(current, pageCount - 1));
  }, [pageCount]);

  const pageApplications = useMemo(
    () =>
      filteredApplications.slice(
        page * APPLICATIONS_PER_PAGE,
        (page + 1) * APPLICATIONS_PER_PAGE,
      ),
    [filteredApplications, page],
  );

  const openToolMenu = useCallback(
    async (appId: number, showMenu: () => void, isFocused: () => boolean) => {
      // Lazily recheck unconfirmed changes whenever the user interacts again.
      if (Object.keys(pendingAssignments).length > 0) {
        requestLatestState();
      }
      const options = toolOptions[appId];
      if (options?.status === "ready") {
        showMenu();
        return;
      }
      if (options?.status === "loading") {
        return;
      }

      const generation = toolCacheGeneration.current;
      setToolOptions((current) => ({
        ...current,
        [appId]: { status: "loading" },
      }));

      try {
        const tools = await GetAvailableCompatTools(appId);
        if (generation !== toolCacheGeneration.current) {
          return;
        }
        setToolOptions((current) => ({
          ...current,
          [appId]: { status: "ready", tools },
        }));
        requestAnimationFrame(() => {
          if (generation === toolCacheGeneration.current && isFocused()) {
            showMenu();
          }
        });
      } catch (error) {
        if (generation === toolCacheGeneration.current) {
          setToolOptions((current) => ({
            ...current,
            [appId]: { status: "error", message: errorMessage(error) },
          }));
        }
      }
    },
    [pendingAssignments, requestLatestState, toolOptions],
  );

  const selectCompatTool = (
    appId: number,
    selectedOption: SingleDropdownOption,
  ) => {
    const nextToolName = String(selectedOption.data ?? "");
    if (compatibilityMappingsStale || nextToolName === assignmentFor(appId)) {
      return;
    }

    try {
      SpecifyCompatTool(appId, nextToolName);
    } catch (error) {
      setAssignmentErrors((current) => ({
        ...current,
        [appId]: message("applications-assignmentFailed", {
          error: errorMessage(error),
        }),
      }));
      return;
    }

    setAssignmentErrors((current) => {
      const next = { ...current };
      delete next[appId];
      return next;
    });
    setPendingAssignments((current) => ({ ...current, [appId]: nextToolName }));
    requestLatestState();
  };

  const pageStart = page * APPLICATIONS_PER_PAGE;
  const shownStart = filteredApplications.length === 0 ? 0 : pageStart + 1;
  const shownEnd = Math.min(
    pageStart + APPLICATIONS_PER_PAGE,
    filteredApplications.length,
  );
  const visibleInventoryWarning =
    source === "steam" ? undefined : inventoryWarning;
  const needsRetry =
    loadError != null ||
    compatibilityMappingsStale ||
    visibleInventoryWarning != null;
  const emptyMessage =
    source === "steam"
      ? "applications-emptySteam"
      : source === "shortcuts"
        ? "applications-emptyShortcuts"
        : "applications-empty";

  const content = (
    <DialogBody>
      <DialogControlsSection>
        {compatibilityMappingsStale && (
          <div role="alert" style={{ color: "#f5c56b", paddingBottom: "10px" }}>
            {t("applications-staleMappings")}
          </div>
        )}
        <Focusable
          flow-children="row"
          style={{
            display: "flex",
            alignItems: "center",
            gap: "10px",
          }}
        >
          <div ref={searchFieldRef} style={{ flex: 1, minWidth: 0 }}>
            <TextField
              aria-label={t("applications-searchLabel")}
              {...{ placeholder: t("applications-searchPlaceholder") }}
              value={search}
              bShowClearAction
              onChange={(event) => {
                setSearch(event.currentTarget.value);
                setPage(0);
              }}
            />
          </div>
          <Focusable
            flow-children="row"
            style={{
              width: "clamp(160px, 23vw, 260px)",
              flex: "0 0 auto",
              boxShadow: "none",
            }}
          >
            <Dropdown
              menuLabel={t("applications-sourceLabel")}
              selectedOption={source}
              rgOptions={[
                { data: "steam", label: t("applications-sourceSteam") },
                { data: "shortcuts", label: t("applications-sourceShortcuts") },
                { data: "all", label: t("applications-sourceAll") },
              ]}
              renderButtonValue={(label) => (
                <span
                  style={{
                    display: "block",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {label}
                </span>
              )}
              onChange={(option) => {
                setSource(option.data as ApplicationSource);
                setPage(0);
              }}
            />
          </Focusable>
        </Focusable>
        {isLoading && applications.length === 0 && (
          <div role="status" aria-live="polite">
            {t("applications-loading")}
          </div>
        )}

        {loadError != null && (
          <div role="alert" style={{ color: "#ff9f9f", paddingBottom: "10px" }}>
            {applications.length === 0
              ? t("applications-loadFailed", { error: loadError })
              : t("applications-refreshFailed", { error: loadError })}
          </div>
        )}

        {visibleInventoryWarning != null && (
          <div
            role="status"
            style={{ color: "#f5c56b", paddingBottom: "10px" }}
          >
            {translateMessage(visibleInventoryWarning)}
          </div>
        )}

        {needsRetry && (
          <DialogButton
            aria-label={t("applications-retryLabel")}
            disabled={isLoading}
            onClick={refresh}
            style={{ width: "auto", minWidth: "90px", marginBottom: "10px" }}
          >
            {t(isLoading ? "applications-retrying" : "applications-retry")}
          </DialogButton>
        )}

        {!isLoading &&
          loadError == null &&
          filteredApplications.length === 0 &&
          !(
            visibleInventoryWarning != null && sourceApplications.length === 0
          ) && (
            <div role="status">
              {search.trim() === ""
                ? t(emptyMessage)
                : t("applications-noMatches", { search: search.trim() })}
            </div>
          )}

        {pageApplications.length > 0 && (
          <>
            <div
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                gap: "10px",
                padding: "8px 0",
                boxShadow: "none",
              }}
            >
              <span
                role="status"
                aria-live="polite"
                style={{ opacity: 0.75, fontSize: "14px" }}
              >
                {t("applications-range", {
                  start: shownStart,
                  end: shownEnd,
                  count: filteredApplications.length,
                })}
              </span>
              {pageCount > 1 && (
                <span aria-live="polite" style={{ fontSize: "14px" }}>
                  {t("applications-page", { page: page + 1, count: pageCount })}
                </span>
              )}
            </div>
            <ul
              ref={applicationListRef}
              aria-label={t("applications-listLabel")}
              style={{ listStyle: "none", margin: 0, padding: 0 }}
            >
              {pageApplications.map((application) => {
                const assignedToolName = assignmentFor(application.appId);
                return (
                  <ApplicationRow
                    key={application.appId}
                    application={application}
                    assignedToolName={assignedToolName}
                    assignedToolDisplayName={toolDisplayNameFor(
                      assignedToolName,
                    )}
                    optionsState={toolOptions[application.appId]}
                    isSaving={application.appId in pendingAssignments}
                    assignmentError={assignmentErrors[application.appId]}
                    disabled={compatibilityMappingsStale}
                    onMenuWillOpen={(showMenu, isFocused) => {
                      void openToolMenu(application.appId, showMenu, isFocused);
                    }}
                    onChange={(option) =>
                      selectCompatTool(application.appId, option)
                    }
                  />
                );
              })}
            </ul>
          </>
        )}
      </DialogControlsSection>
    </DialogBody>
  );

  return (
    <Focusable
      flow-children="column"
      style={{ boxShadow: "none" }}
      onOptionsButton={focusSearch}
      onOptionsActionDescription={t("applications-search")}
      onButtonDown={handlePageButton}
      actionDescriptionMap={{
        [GamepadButton.BUMPER_LEFT]: canGoToPreviousPage
          ? t("applications-previous")
          : undefined,
        [GamepadButton.BUMPER_RIGHT]: canGoToNextPage
          ? t("applications-next")
          : undefined,
      }}
    >
      {content}
    </Focusable>
  );
}
