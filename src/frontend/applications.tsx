import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  Dropdown,
  Field,
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
import { FaChevronLeft, FaChevronRight } from "react-icons/fa";
import { AppState } from "../types";
import { requestState } from "../utils/backendApi";
import {
  DisplayMessage,
  LocalizedMessage,
  errorMessage,
  message,
  useTranslation,
} from "../i18n";
import {
  CompatToolInfo,
  GetAvailableCompatTools,
  GetManagedApplications,
  ManagedSteamApplication,
  SpecifyCompatTool,
} from "../utils/steamUtils";

const APPLICATIONS_PER_PAGE = 40;

interface ApplicationsTabProps {
  appState: AppState;
  socket: WebSocket;
}

interface OptimisticAssignment {
  appId: number;
  toolName: string;
}

type ToolOptionsState =
  | { status: "loading" }
  | { status: "ready"; tools: CompatToolInfo[] }
  | { status: "error"; message: DisplayMessage };

function applicationKey(application: ManagedSteamApplication): string {
  return `${application.isShortcut ? "shortcut" : "steam"}:${application.appId}`;
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
    options.push({
      data: assignedToolName,
      label: assignedToolDisplayName,
    });
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

export default function ApplicationsTab({
  appState,
  socket,
}: ApplicationsTabProps) {
  const { t, translateMessage } = useTranslation();
  const [applications, setApplications] = useState<ManagedSteamApplication[]>(
    [],
  );
  const [search, setSearch] = useState("");
  const searchFieldRef = useRef<HTMLDivElement>(null);
  const applicationListRef = useRef<HTMLUListElement>(null);
  const restoreListFocus = useRef(false);
  const [page, setPage] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<DisplayMessage>();
  const [inventoryWarning, setInventoryWarning] = useState<LocalizedMessage>();
  const [assignmentErrors, setAssignmentErrors] = useState<
    Record<string, DisplayMessage>
  >({});
  const [optimisticAssignments, setOptimisticAssignments] = useState<
    Record<string, OptimisticAssignment>
  >({});
  const [toolOptions, setToolOptions] = useState<
    Record<string, ToolOptionsState>
  >({});
  const loadGeneration = useRef(0);
  const toolCacheGeneration = useRef(0);
  const toolLoads = useRef(new Map<string, Promise<CompatToolInfo[]>>());
  const assignmentVerificationTimers = useRef(
    new Map<string, ReturnType<typeof setTimeout>[]>(),
  );
  const isMounted = useRef(true);
  const visibleApplicationKeys = useRef(new Set<string>());

  const compatibilityMappings = appState.app_compat_tool_mappings ?? {};
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
  const optimisticAssignmentsRef = useRef(optimisticAssignments);
  optimisticAssignmentsRef.current = optimisticAssignments;

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
    toolLoads.current.clear();
    setToolOptions({});
  }, []);

  const requestLatestState = useCallback(() => {
    try {
      requestState(socket);
    } catch {
      // Inventory remains useful while the management socket reconnects.
    }
  }, [socket]);

  const clearAssignmentVerification = useCallback((key: string) => {
    assignmentVerificationTimers.current
      .get(key)
      ?.forEach((timer) => clearTimeout(timer));
    assignmentVerificationTimers.current.delete(key);
  }, []);

  const refresh = useCallback(() => {
    resetToolOptions();
    requestLatestState();
    void loadApplications();
  }, [loadApplications, requestLatestState, resetToolOptions]);

  useEffect(() => {
    isMounted.current = true;
    requestLatestState();
    void loadApplications();

    return () => {
      isMounted.current = false;
      loadGeneration.current += 1;
      toolCacheGeneration.current += 1;
      toolLoads.current.clear();
      assignmentVerificationTimers.current.forEach((timers) =>
        timers.forEach((timer) => clearTimeout(timer)),
      );
      assignmentVerificationTimers.current.clear();
    };
  }, [clearAssignmentVerification, loadApplications, requestLatestState]);

  // A backend state update can lag behind Steam's local mutation. Keep the
  // optimistic value through unrelated updates and remove it only once the
  // authoritative compatibility mapping confirms the same value.
  useEffect(() => {
    const confirmedKeys = Object.entries(optimisticAssignmentsRef.current)
      .filter(([, assignment]) => {
        const authoritativeToolName =
          compatibilityMappings[String(assignment.appId)] ?? "";
        return authoritativeToolName === assignment.toolName;
      })
      .map(([key]) => key);

    if (confirmedKeys.length === 0) {
      return;
    }

    confirmedKeys.forEach(clearAssignmentVerification);
    setOptimisticAssignments((current) => {
      const next = { ...current };
      confirmedKeys.forEach((key) => delete next[key]);
      return next;
    });
  }, [clearAssignmentVerification, compatibilityMappings]);

  useEffect(() => {
    setPage(0);
  }, [search]);

  const assignmentFor = useCallback(
    (application: ManagedSteamApplication): string => {
      const optimistic = optimisticAssignments[applicationKey(application)];
      return (
        optimistic?.toolName ??
        compatibilityMappings[String(application.appId)] ??
        ""
      );
    },
    [compatibilityMappings, optimisticAssignments],
  );

  const filteredApplications = useMemo(() => {
    const normalizedSearch = search.trim().toLocaleLowerCase();
    if (normalizedSearch === "") {
      return applications;
    }

    return applications.filter((application) => {
      const key = applicationKey(application);
      const assignedToolName = assignmentFor(application);
      const selectedTool =
        toolOptions[key]?.status === "ready"
          ? toolOptions[key].tools.find(
              (tool) => tool.strToolName === assignedToolName,
            )
          : undefined;
      const assignedToolDisplayName =
        selectedTool?.strDisplayName || toolDisplayNames.get(assignedToolName) || "";

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
  }, [applications, assignmentFor, search, toolDisplayNames, toolOptions, t]);

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
    setPage((current) => Math.max(0, Math.min(pageCount - 1, current + offset)));
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

  visibleApplicationKeys.current = new Set(
    pageApplications.map(applicationKey),
  );

  const openToolMenu = useCallback(
    async (
      application: ManagedSteamApplication,
      showMenu: () => void,
    ) => {
      const key = applicationKey(application);
      if (toolOptions[key]?.status === "ready") {
        showMenu();
        return;
      }

      if (toolLoads.current.has(key)) {
        return;
      }

      const generation = toolCacheGeneration.current;
      setToolOptions((current) => ({
        ...current,
        [key]: { status: "loading" },
      }));

      const request = Promise.resolve().then(() =>
        GetAvailableCompatTools(application.appId),
      );
      toolLoads.current.set(key, request);

      try {
        const tools = await request;
        if (!isMounted.current || generation !== toolCacheGeneration.current) {
          return;
        }

        setToolOptions((current) => ({
          ...current,
          [key]: { status: "ready", tools },
        }));

        requestAnimationFrame(() => {
          if (
            isMounted.current &&
            generation === toolCacheGeneration.current &&
            visibleApplicationKeys.current.has(key)
          ) {
            showMenu();
          }
        });
      } catch (error) {
        if (isMounted.current && generation === toolCacheGeneration.current) {
          setToolOptions((current) => ({
            ...current,
            [key]: {
              status: "error",
              message: errorMessage(error),
            },
          }));
        }
      } finally {
        if (toolLoads.current.get(key) === request) {
          toolLoads.current.delete(key);
        }
      }
    },
    [toolOptions],
  );

  const selectCompatTool = useCallback(
    (
      application: ManagedSteamApplication,
      selectedOption: SingleDropdownOption,
    ) => {
      const nextToolName = String(selectedOption.data ?? "");
      const key = applicationKey(application);
      const previousToolName = assignmentFor(application);
      if (compatibilityMappingsStale || nextToolName === previousToolName) {
        return;
      }

      const previousOptimisticAssignment = optimisticAssignments[key];
      setAssignmentErrors((current) => {
        const next = { ...current };
        delete next[key];
        return next;
      });
      setOptimisticAssignments((current) => ({
        ...current,
        [key]: { appId: application.appId, toolName: nextToolName },
      }));

      try {
        SpecifyCompatTool(application.appId, nextToolName);
      } catch (error) {
        setOptimisticAssignments((current) => {
          if (current[key]?.toolName !== nextToolName) {
            return current;
          }

          const next = { ...current };
          if (previousOptimisticAssignment == null) {
            delete next[key];
          } else {
            next[key] = previousOptimisticAssignment;
          }
          return next;
        });
        setAssignmentErrors((current) => ({
          ...current,
          [key]: message("applications-assignmentFailed", {
            error: errorMessage(error),
          }),
        }));
        return;
      }

      clearAssignmentVerification(key);
      const verificationTimers = [
        setTimeout(requestLatestState, 500),
        setTimeout(requestLatestState, 1_500),
        setTimeout(requestLatestState, 2_750),
        setTimeout(() => {
          const pendingAssignment = optimisticAssignmentsRef.current[key];
          if (pendingAssignment?.toolName !== nextToolName) {
            clearAssignmentVerification(key);
            return;
          }

          // Stop presenting an unconfirmed optimistic value. The row now falls
          // back to the newest backend snapshot; a delayed response can still
          // update it without leaving behind a false failure message.
          setOptimisticAssignments((current) => {
            if (current[key]?.toolName !== nextToolName) {
              return current;
            }

            const next = { ...current };
            delete next[key];
            return next;
          });
          clearAssignmentVerification(key);
        }, 4_000),
      ];
      assignmentVerificationTimers.current.set(key, verificationTimers);
    },
    [
      assignmentFor,
      clearAssignmentVerification,
      compatibilityMappingsStale,
      optimisticAssignments,
      requestLatestState,
    ],
  );

  const pageStart = page * APPLICATIONS_PER_PAGE;
  const shownStart = filteredApplications.length === 0 ? 0 : pageStart + 1;
  const shownEnd = Math.min(
    pageStart + APPLICATIONS_PER_PAGE,
    filteredApplications.length,
  );

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
          <div
            ref={searchFieldRef}
            style={{ flex: 1, minWidth: 0 }}
          >
            <TextField
              aria-label={t("applications-searchLabel")}
              {...{ placeholder: t("applications-searchPlaceholder") }}
              value={search}
              bShowClearAction
              onChange={(event) => setSearch(event.currentTarget.value)}
            />
          </div>
          <DialogButton
            aria-label={t("applications-refreshLabel")}
            disabled={isLoading}
            onClick={refresh}
            style={{
              width: "auto",
              minWidth: "90px",
              flex: "0 0 auto",
            }}
          >
            {isLoading ? t("common-refreshing") : t("common-refresh")}
          </DialogButton>
        </Focusable>
        {isLoading && applications.length === 0 && (
          <div role="status" aria-live="polite">
            {t("applications-loading")}
          </div>
        )}

        {loadError != null && (
          <div
            role="alert"
            style={{ color: "#ff9f9f", paddingBottom: "10px" }}
          >
            {applications.length === 0
              ? t("applications-loadFailed", { error: loadError })
              : t("applications-refreshFailed", { error: loadError })}
          </div>
        )}

        {inventoryWarning != null && (
          <div role="status" style={{ color: "#f5c56b", paddingBottom: "10px" }}>
            {translateMessage(inventoryWarning)}
          </div>
        )}

        {!isLoading && loadError == null && applications.length === 0 && (
          <div>{t("applications-empty")}</div>
        )}

        {applications.length > 0 && filteredApplications.length === 0 && (
          <div role="status">
            {t("applications-noMatches", { search: search.trim() })}
          </div>
        )}

        {pageApplications.length > 0 && (
          <>
            <Focusable
              flow-children="row"
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
                <div
                  style={{ display: "flex", alignItems: "center", gap: "8px" }}
                >
                  <DialogButton
                    aria-label={t("applications-previousLabel")}
                    onOKActionDescription={t("applications-previous")}
                    disabled={!canGoToPreviousPage}
                    onClick={previousPage}
                    style={{
                      width: "40px",
                      minWidth: "40px",
                      height: "32px",
                      padding: "4px 8px",
                    }}
                  >
                    <FaChevronLeft aria-hidden="true" />
                  </DialogButton>
                  <span aria-live="polite" style={{ fontSize: "14px" }}>
                    {t("applications-page", { page: page + 1, count: pageCount })}
                  </span>
                  <DialogButton
                    aria-label={t("applications-nextLabel")}
                    onOKActionDescription={t("applications-next")}
                    disabled={!canGoToNextPage}
                    onClick={nextPage}
                    style={{
                      width: "40px",
                      minWidth: "40px",
                      height: "32px",
                      padding: "4px 8px",
                    }}
                  >
                    <FaChevronRight aria-hidden="true" />
                  </DialogButton>
                </div>
              )}
            </Focusable>
            <ul
              ref={applicationListRef}
              aria-label={t("applications-listLabel")}
              style={{ listStyle: "none", margin: 0, padding: 0 }}
            >
              {pageApplications.map((application) => {
                const key = applicationKey(application);
                const assignmentError = assignmentErrors[key];
                const optionsState = toolOptions[key];
                const assignedToolName = assignmentFor(application);
                const availableTools =
                  optionsState?.status === "ready" ? optionsState.tools : [];
                const assignedToolDisplayName =
                  assignedToolName === ""
                    ? t("applications-steamDefault")
                    : availableTools.find(
                        (tool) => tool.strToolName === assignedToolName,
                      )?.strDisplayName ||
                      toolDisplayNames.get(assignedToolName) ||
                      assignedToolName;

                return (
                  <li key={key} style={{ margin: 0, padding: 0 }}>
                    <Field
                      label={
                        <div style={{ minWidth: 0 }}>
                          <div
                            title={t("applications-appTitle", {
                              name: application.name || t("applications-appName", { id: String(application.appId) }),
                              id: String(application.appId),
                            })}
                            style={{
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                            }}
                          >
                            {application.name || t("applications-appName", { id: String(application.appId) })}
                          </div>
                          <div
                            style={{
                              fontSize: "12px",
                              lineHeight: "16px",
                              opacity: 0.65,
                            }}
                          >
                            {application.isShortcut
                              ? t("applications-shortcut")
                              : t("applications-steamGame")}
                          </div>
                        </div>
                      }
                      description={
                        (optionsState?.status === "loading" ||
                          optionsState?.status === "error" ||
                          optimisticAssignments[key] != null ||
                          assignmentError != null) && (
                          <div>
                            {optionsState?.status === "loading" && (
                              <div role="status">{t("applications-loadingTools")}</div>
                            )}
                            {optionsState?.status === "error" && (
                              <div role="alert" style={{ color: "#ff9f9f" }}>
                                {t("applications-toolsFailed", { error: optionsState.message })}
                              </div>
                            )}
                            {optimisticAssignments[key] != null && (
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
                          disabled={compatibilityMappingsStale}
                          menuLabel={t("applications-toolFor", {
                            name: application.name || t("applications-appName", { id: String(application.appId) }),
                          })}
                          focusable
                          renderButtonValue={() => (
                            <span
                              aria-label={t("applications-selectedTool", {
                                name: application.name || t("applications-appName", { id: String(application.appId) }),
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
                          onMenuWillOpen={(showMenu) => {
                            void openToolMenu(application, showMenu);
                          }}
                          onChange={(option) =>
                            selectCompatTool(application, option)
                          }
                        />
                      </Focusable>
                    </Field>
                  </li>
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
        [GamepadButton.BUMPER_LEFT]: canGoToPreviousPage ? t("applications-previous") : undefined,
        [GamepadButton.BUMPER_RIGHT]: canGoToNextPage ? t("applications-next") : undefined,
      }}
    >
      {content}
    </Focusable>
  );
}
