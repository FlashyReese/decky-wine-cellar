import { Markdown } from "./markdown";
import { GitHubRelease } from "../types";
import { Focusable, ScrollPanelGroup } from "@decky/ui";

// Decky's public type only declares `children`, but Steam's component also
// forwards the normal Focusable/div props used here for sizing and autofocus.
const ChangelogScrollPanel = ScrollPanelGroup as typeof Focusable;

function ChangeLogModal({
  release,
  closeModal,
}: {
  release: GitHubRelease;
  closeModal?: () => void;
}) {
  return (
    <Focusable
      onCancelButton={closeModal}
      style={{ height: "100%", minHeight: 0 }}
    >
      <ChangelogScrollPanel
        autoFocus
        style={{
          margin: "40px",
          width: "calc(100% - 80px)",
          height: "calc(100% - 80px)",
          minHeight: 0,
          padding: "0 16px",
          boxSizing: "border-box",
        }}
      >
        <div
          style={{
            width: "100%",
            maxWidth: "900px",
            margin: "0 auto",
            paddingBottom: "40px",
            overflowWrap: "anywhere",
          }}
        >
          <h1>{release.name}</h1>
          {release.body ? (
            <Markdown onDismiss={closeModal}>{release.body}</Markdown>
          ) : (
            "no patch notes for this version"
          )}
        </div>
      </ChangelogScrollPanel>
    </Focusable>
  );
}

export default ChangeLogModal;
