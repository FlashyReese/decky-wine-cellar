import {
  DialogBody,
  DialogButton,
  DialogControlsSection,
  DialogControlsSectionHeader,
  Dropdown,
  Field,
  Focusable,
  SingleDropdownOption,
  TextField,
} from "@decky/ui";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppState } from "../types";
import { requestState } from "../utils/backendApi";
import {
  CompatToolInfo,
  GetAvailableCompatTools,
  GetManagedApplications,
  ManagedSteamApplication,
  SpecifyCompatTool,
} from "../utils/steamUtils";

const APPLICATIONS_PER_PAGE = 40;

const STEAM_DEFAULT_OPTION: SingleDropdownOption = {
  data: "",
  label: "Steam default",
};

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
  | { status: "error"; message: string };

function applicationKey(application: ManagedSteamApplication): string {
  return `${application.isShortcut ? "shortcut" : "steam"}:${application.appId}`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function dropdownOptions(
  tools: CompatToolInfo[],
  assignedToolName: string,
  assignedToolDisplayName: string,
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

  return [STEAM_DEFAULT_OPTION, ...options];
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
        width: "40px",
        minWidth: "40px",
        height: "40px",
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
            width: "40px",
            height: "40px",
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
  const [applications, setApplications] = useState<ManagedSteamApplication[]>(
    [],
  );
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string>();
  const [inventoryWarning, setInventoryWarning] = useState<string>();
  const [assignmentErrors, setAssignmentErrors] = useState<
    Record<string, string>
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
        application.isShortcut ? "non-steam shortcut" : "steam game",
      ].some((value) => value.toLocaleLowerCase().includes(normalizedSearch));
    });
  }, [applications, assignmentFor, search, toolDisplayNames, toolOptions]);

  const pageCount = Math.max(
    1,
    Math.ceil(filteredApplications.length / APPLICATIONS_PER_PAGE),
  );

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
          [key]: `Could not change the compatibility tool: ${errorMessage(error)}`,
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

  return (
    <DialogBody>
      <DialogControlsSection>
        <DialogControlsSectionHeader>
          Application Compatibility
        </DialogControlsSectionHeader>
        <p style={{ marginTop: 0 }}>
          Choose the compatibility tool for installed Steam games and non-Steam
          shortcuts. Select Steam default to remove a manual override.
        </p>
        {compatibilityMappingsStale && (
          <div role="alert" style={{ color: "#f5c56b", paddingBottom: "10px" }}>
            Saved compatibility assignments could not be refreshed. Changes are
            disabled until Refresh succeeds.
          </div>
        )}
        <Focusable
          flow-children="row"
          style={{
            display: "flex",
            alignItems: "flex-end",
            flexWrap: "wrap",
            gap: "10px",
          }}
        >
          <div style={{ flex: "1 1 280px", minWidth: 0 }}>
            <TextField
              label="Search applications"
              aria-label="Search installed Steam games and non-Steam shortcuts"
              value={search}
              bShowClearAction
              onChange={(event) => setSearch(event.currentTarget.value)}
            />
          </div>
          <DialogButton
            aria-label="Refresh installed applications and compatibility assignments"
            disabled={isLoading}
            onClick={refresh}
            style={{
              width: "auto",
              minWidth: "110px",
              flex: "0 0 auto",
            }}
          >
            {isLoading ? "Refreshing…" : "Refresh"}
          </DialogButton>
        </Focusable>
      </DialogControlsSection>

      <DialogControlsSection>
        <DialogControlsSectionHeader>
          Installed Applications
        </DialogControlsSectionHeader>

        {isLoading && applications.length === 0 && (
          <div role="status" aria-live="polite">
            Loading installed applications…
          </div>
        )}

        {loadError != null && (
          <div
            role="alert"
            style={{ color: "#ff9f9f", paddingBottom: "10px" }}
          >
            {applications.length === 0
              ? `Could not load applications: ${loadError}`
              : `Could not refresh applications: ${loadError}. Previously loaded results are still shown.`}
          </div>
        )}

        {inventoryWarning != null && (
          <div role="status" style={{ color: "#f5c56b", paddingBottom: "10px" }}>
            {inventoryWarning}
          </div>
        )}

        {!isLoading && loadError == null && applications.length === 0 && (
          <div>No installed Steam games or non-Steam shortcuts were found.</div>
        )}

        {applications.length > 0 && filteredApplications.length === 0 && (
          <div role="status">No applications match “{search.trim()}”.</div>
        )}

        {pageApplications.length > 0 && (
          <>
            <div
              role="status"
              aria-live="polite"
              style={{ opacity: 0.75, paddingBottom: "8px" }}
            >
              Showing {shownStart}–{shownEnd} of {filteredApplications.length}
            </div>
            <ul
              aria-label="Installed Steam games and non-Steam shortcuts"
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
                    ? "Steam default"
                    : availableTools.find(
                        (tool) => tool.strToolName === assignedToolName,
                      )?.strDisplayName ||
                      toolDisplayNames.get(assignedToolName) ||
                      assignedToolName;

                return (
                  <li key={key} style={{ margin: 0, padding: 0 }}>
                    <Field
                      label={application.name || `App ${application.appId}`}
                      description={
                        <div>
                          <div>
                            {application.isShortcut
                              ? "Non-Steam shortcut"
                              : "Steam game"}
                            {` · App ID ${application.appId}`}
                          </div>
                          {optionsState?.status === "loading" && (
                            <div role="status">Loading compatible tools…</div>
                          )}
                          {optionsState?.status === "error" && (
                            <div role="alert" style={{ color: "#ff9f9f" }}>
                              Could not load compatible tools: {optionsState.message}.
                              Select the menu again to retry.
                            </div>
                          )}
                          {optimisticAssignments[key] != null && (
                            <div role="status">Saving compatibility override…</div>
                          )}
                          {assignmentError != null && (
                            <div role="alert" style={{ color: "#ff9f9f" }}>
                              {assignmentError}
                            </div>
                          )}
                        </div>
                      }
                      icon={<ApplicationIcon application={application} />}
                      bottomSeparator="standard"
                      inlineWrap="shift-children-below"
                      childrenContainerWidth="min"
                      verticalAlignment="center"
                    >
                      <Focusable
                        flow-children="row"
                        style={{
                          width: "clamp(190px, 32vw, 340px)",
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
                          )}
                          selectedOption={assignedToolName}
                          disabled={compatibilityMappingsStale}
                          menuLabel={`Compatibility tool for ${application.name || `App ${application.appId}`}`}
                          focusable
                          renderButtonValue={() => (
                            <span
                              aria-label={`Compatibility tool for ${application.name || `App ${application.appId}`}: ${assignedToolDisplayName}`}
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

            {pageCount > 1 && (
              <Focusable
                flow-children="row"
                style={{
                  display: "flex",
                  alignItems: "center",
                  justifyContent: "space-between",
                  gap: "10px",
                  paddingTop: "12px",
                }}
              >
                <DialogButton
                  aria-label="Previous application page"
                  disabled={page === 0}
                  onClick={() => setPage((current) => Math.max(0, current - 1))}
                  style={{ width: "auto", minWidth: "110px" }}
                >
                  Previous
                </DialogButton>
                <span aria-live="polite">
                  Page {page + 1} of {pageCount}
                </span>
                <DialogButton
                  aria-label="Next application page"
                  disabled={page + 1 >= pageCount}
                  onClick={() =>
                    setPage((current) => Math.min(pageCount - 1, current + 1))
                  }
                  style={{ width: "auto", minWidth: "110px" }}
                >
                  Next
                </DialogButton>
              </Focusable>
            )}
          </>
        )}
      </DialogControlsSection>
    </DialogBody>
  );
}
