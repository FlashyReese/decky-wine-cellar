import { ProgressBar } from "@decky/ui";
import { OperationInfo, OperationState } from "../types";

export default function OperationProgress({
  operation,
  showLabel = false,
}: {
  operation: OperationInfo;
  showLabel?: boolean;
}) {
  const downloading = operation.state === OperationState.Downloading;
  const download = downloading ? operation.download : null;
  const totalBytes = download?.total_bytes;
  const hasTotal = totalBytes != null && totalBytes > 0;
  const progress = Math.min(100, Math.max(0, operation.progress));
  const rate = download?.bytes_per_second;
  const eta = download?.eta_seconds;
  const pending = operation.state === OperationState.Pending;

  return (
    <div style={{ width: "100%", minWidth: 0, paddingTop: "8px" }}>
      {showLabel && (
        <div style={{ paddingBottom: "6px", overflowWrap: "anywhere" }}>
          {operation.label}
        </div>
      )}
      <div style={{ paddingBottom: "6px" }}>
        {pending ? "Queued" : operation.state}
        {downloading && hasTotal && ` · ${progress}%`}
      </div>
      {!pending && (
        <ProgressBar
          nProgress={downloading ? progress : 0}
          indeterminate={!downloading || !hasTotal}
          focusable={false}
        />
      )}
      {downloading && (
        <div
          style={{
            display: "flex",
            flexWrap: "wrap",
            columnGap: "16px",
            rowGap: "4px",
            paddingTop: "8px",
            fontSize: "14px",
            opacity: 0.8,
          }}
        >
          <span>
            {formatBytes(download?.bytes_downloaded ?? 0)}
            {hasTotal ? ` / ${formatBytes(totalBytes)}` : " downloaded"}
          </span>
          <span>
            {rate != null && rate > 0
              ? `${(rate / (1024 * 1024)).toFixed(rate < 1024 * 1024 ? 2 : 1)} MiB/s`
              : rate === 0
                ? "Waiting for data…"
                : "Estimating speed…"}
          </span>
          <span>
            {hasTotal
              ? eta != null && eta >= 0
                ? `ETA ${formatDuration(eta)}`
                : "Estimating time remaining…"
              : "Time remaining unknown"}
          </span>
          {download != null && (
            <span>{formatDuration(download.elapsed_seconds)} elapsed</span>
          )}
        </div>
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  const value = Math.max(0, bytes);
  if (value < 1024) {
    return `${Math.round(value)} B`;
  }
  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KiB`;
  }
  if (value < 1024 * 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
  }
  return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GiB`;
}

function formatDuration(seconds: number): string {
  const duration = Math.max(0, Math.ceil(seconds));
  if (duration < 60) {
    return `${duration}s`;
  }
  const minutes = Math.floor(duration / 60);
  if (minutes < 60) {
    return `${minutes}m ${duration % 60}s`;
  }
  return `${Math.floor(minutes / 60)}h ${minutes % 60}m`;
}
