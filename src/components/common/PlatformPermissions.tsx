import type { PermissionKind, PlatformStatus } from "../../utils/platform";
import { permissionRequirements } from "../../utils/platform";
const permissions: { kind: PermissionKind; title: string; description: string }[] = [
  { kind: "microphone", title: "麦克风", description: "录制你按下快捷键时的语音" },
  { kind: "accessibility", title: "辅助功能", description: "读取选中文本，并将结果粘贴回原输入位置" },
  { kind: "input_monitoring", title: "输入监控", description: "在其他应用中响应全局快捷键" },
];
export function PlatformPermissions({ platform, error, requesting, onRequest, onRefresh, serviceIdle, startingService, onStartService }: {
  platform: PlatformStatus | null;
  error: string | null;
  requesting: PermissionKind | null;
  onRequest: (permission: PermissionKind) => Promise<void>;
  onRefresh: () => Promise<void>;
  serviceIdle: boolean;
  startingService: boolean;
  onStartService: () => Promise<void>;
}) {
  if (platform?.os === "windows") return null;
  const ready = platform !== null && permissionRequirements(platform).length === 0;
  return (
    <section aria-label="系统权限" className="space-y-3">
      <div className="flex items-center justify-between gap-4">
        <h2 className="text-sm font-bold text-[var(--ink)]">系统权限</h2>
        <button type="button" onClick={() => void onRefresh()} className="rounded-lg px-3 py-2 text-sm text-[var(--ink)] underline underline-offset-4 focus-visible:outline-2 focus-visible:outline-[var(--crail)]">刷新状态</button>
      </div>
      <p className="text-sm text-stone-600" role="status">
        {error || (!platform ? "正在读取权限状态…" : permissionRequirements(platform).length > 0
          ? "完成三项授权后，刷新状态并启动服务。系统要求重新打开应用时，请退出后重开。"
          : "权限已就绪。服务启动后，可在其他应用中使用录音快捷键。")}
      </p>
      {platform && permissions.map(({ kind, title, description }) => (
        <div key={kind} className="flex flex-wrap items-center justify-between gap-3 border-b border-[var(--stone)] py-3 last:border-b-0">
          <div className="min-w-0">
            <div className="text-sm font-semibold text-[var(--ink)]">{title}</div>
            <p className="text-sm text-stone-600">{description}</p>
          </div>
          {platform[kind] === "granted" ? <span className="text-sm text-green-800">已允许</span> : (
            <button type="button" disabled={requesting !== null || platform[kind] === "restricted"}
              onClick={() => void onRequest(kind)} aria-label={`设置${title}权限`}
              className="rounded-lg border border-[var(--stone)] bg-[var(--paper)] px-3 py-2 text-sm font-semibold text-[var(--ink)] hover:border-[var(--crail)] focus-visible:outline-2 focus-visible:outline-[var(--crail)] disabled:cursor-not-allowed disabled:opacity-50">
              {platform[kind] === "restricted" ? "系统限制" : requesting === kind ? "正在打开…" : "去授权"}
            </button>
          )}
        </div>
      ))}
      {serviceIdle && <button type="button" disabled={!ready || requesting !== null || startingService}
        onClick={() => void onStartService()}
        className="rounded-lg bg-[var(--ink)] px-4 py-2 text-sm font-semibold text-[var(--paper)] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--crail)] disabled:cursor-not-allowed disabled:opacity-50">
        {startingService ? "正在启动…" : "启动服务"}
      </button>}
    </section>
  );
}
