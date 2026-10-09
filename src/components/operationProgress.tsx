import { ProgressBar } from "@decky/ui";
import { OperationInfo, OperationState } from "../types";
import { locales, Translator, useTranslation } from "../i18n";

export default function OperationProgress({
  operation,
  showLabel = false,
}: {
  operation: OperationInfo;
  showLabel?: boolean;
}) {
  const { t, locale, translateMessage } = useTranslation();
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
          {translateMessage(operation.label)}
        </div>
      )}
      <div style={{ paddingBottom: "6px" }}>
        {t(`operation-state-${operation.state}`)}
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
            {hasTotal
              ? `${formatBytes(download?.bytes_downloaded ?? 0)} / ${formatBytes(totalBytes)}`
              : t("download-downloaded", {
                  bytes: formatBytes(download?.bytes_downloaded ?? 0),
                })}
          </span>
          <span>
            {rate != null && rate > 0
              ? t("download-speed", {
                  speed: (rate / (1024 * 1024)).toLocaleString(locales[locale].tag, {
                    minimumFractionDigits: rate < 1024 * 1024 ? 2 : 1,
                    maximumFractionDigits: rate < 1024 * 1024 ? 2 : 1,
                  }),
                })
              : rate === 0
                ? t("download-waiting")
                : t("download-estimatingSpeed")}
          </span>
          <span>
            {hasTotal
              ? eta != null && eta >= 0
                ? t("download-eta", { duration: formatDuration(eta, t) })
                : t("download-estimatingTime")
              : t("download-timeUnknown")}
          </span>
          {download != null && (
            <span>
              {t("download-elapsed", {
                duration: formatDuration(download.elapsed_seconds, t),
              })}
            </span>
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

function formatDuration(seconds: number, t: Translator): string {
  const duration = Math.max(0, Math.ceil(seconds));
  if (duration < 60) {
    return t("duration-seconds", { seconds: duration });
  }
  const minutes = Math.floor(duration / 60);
  if (minutes < 60) {
    return t("duration-minutes", { minutes, seconds: duration % 60 });
  }
  return t("duration-hours", {
    hours: Math.floor(minutes / 60),
    minutes: minutes % 60,
  });
}
