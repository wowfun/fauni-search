import type {
  ApiErrorPayload,
  AppState,
  BindingSource,
  ContentTypeBindingPayload,
  ContentTypesPayload,
  EmbeddingCapabilities,
  GlobalContentTypesData,
  ImportPathsData,
  InventorySummary,
  JobSnapshot,
  JobsListData,
  LibrariesListData,
  LibraryContentTypesData,
  LibraryObjectQueryDocument,
  LibraryObjectQueryImage,
  LibraryObjectQueryVideo,
  LibrarySnapshot,
  MaintenanceActionData,
  ModelCatalogData,
  ModelCatalogEntry,
  ModelTestData,
  ModelTestModality,
  ModelSelectionPayload,
  PreviewReference,
  ProviderConfigSnapshot,
  ProvidersListData,
  QueryAssetData,
  ResolvedContentModelSelectionPayload,
  ResolvedContentModelsData,
  ResolvedModelSelectionPayload,
  RuntimeHealthData,
  SearchMode,
  SearchOutcomeState,
  SearchRequestSnapshot,
  SearchScopeKind,
  SourceActionData,
  SourceInventoryItem,
  SourceRootSnapshot,
  SourceRootsListData,
  SettingsSection,
  SourcesListData,
  VectorSpaceDiagnosticsData,
  VideoSourceItem,
  VideoSourcesData,
  AssetDetailData,
  WorkspaceKind,
} from "../../types";
import { state } from "../state/store";
import {
  formatBindingSource,
  formatResolvedContentModel,
  formatResolvedContentModelContext,
  libraryDisplayName,
  providerSelectionPillClass,
} from "./common";
import {
  availableContentTypeKeys,
  composeModelReference,
  defaultContentTypeBinding,
  libraryContentTypeHasOverride,
  selectionFromBinding,
} from "./settings";

export function runtimeHealthOverview() {
  if (!state.runtimeHealth) {
    return null;
  }

  const processSnapshots = [state.runtimeHealth.app, state.runtimeHealth.qdrant];
  const processIssues = processSnapshots.filter((snapshot) => snapshot.status !== "available");
  const enabledProviders = state.runtimeHealth.providers.filter((provider) => provider.enabled);
  const providerIssues = enabledProviders.filter((provider) => provider.status !== "available");

  return {
    processSnapshots,
    processIssues,
    enabledProviders,
    providerIssues,
    summary:
      processIssues.length || providerIssues.length
        ? `运行时有 ${processIssues.length + providerIssues.length} 个受限项，建议打开诊断查看详细状态。`
        : enabledProviders.length
          ? `运行时健康，${enabledProviders.length} 个已启用连接当前可用。`
          : "运行时健康，当前没有启用中的连接异常。",
  };
}

export function shellRuntimeStatusLabel(status: string) {
  if (status === "available") {
    return "正常";
  }
  if (status === "not_enabled") {
    return "未启用";
  }
  if (status === "runtime_unavailable" || status === "not_supported") {
    return "受限";
  }
  return "待确认";
}

export function globalJobsProgressSummary() {
  const activeJobs = state.globalJobs.filter(
    (job) => job.status === "queued" || job.status === "running"
  );
  if (!activeJobs.length) {
    return null;
  }

  let completed = 0;
  let total = 0;
  const units = new Set<string>();

  activeJobs.forEach((job) => {
    if (job.progress.total > 0) {
      total += job.progress.total;
      completed += Math.min(Math.max(job.progress.completed, 0), job.progress.total);
      if (job.progress.unit) {
        units.add(job.progress.unit);
      }
    }
  });

  const percent = total > 0 ? Math.round((completed / total) * 100) : null;
  const unit = units.size === 1 ? Array.from(units)[0] : "";

  return {
    count: activeJobs.length,
    completed,
    total,
    percent,
    unit,
    label: total > 0 ? (unit ? `${completed}/${total} ${unit}` : `${percent}%`) : "等待开始",
    summary: `所有库有 ${activeJobs.length} 个后台任务正在进行。`,
  };
}

export function currentStatusCapsule(library: LibrarySnapshot | null) {
  const runtimeOverview = runtimeHealthOverview();
  const jobsProgress = globalJobsProgressSummary();

  if (state.globalError) {
    return {
      label: "部分受限",
      pillClass: "error",
      summary: state.globalError.message,
      ...(jobsProgress ? { progress: jobsProgress } : {}),
    };
  }
  if (runtimeOverview && (runtimeOverview.processIssues.length || runtimeOverview.providerIssues.length)) {
    return {
      label: "部分受限",
      pillClass: "error",
      summary: runtimeOverview.summary,
      ...(jobsProgress ? { progress: jobsProgress } : {}),
    };
  }
  if (jobsProgress) {
    return {
      label: `准备中 · ${jobsProgress.count}`,
      pillClass: "pending",
      summary: jobsProgress.summary,
      progress: jobsProgress,
    };
  }
  if (!library) {
    return {
      label: "准备中",
      pillClass: "pending",
      summary: "还没有选定库，先创建或选择一个库。",
    };
  }

  const readiness = libraryOperationalReadiness(library);

  if (library.counts.pending_jobs > 0 && readiness.searchableUnits <= 0) {
    return {
      label: "准备中",
      pillClass: "pending",
      summary: readiness.summary,
    };
  }
  if (readiness.searchableUnits <= 0) {
    return {
      label: readiness.pillClass === "pending" ? "准备中" : "部分受限",
      pillClass: readiness.pillClass,
      summary: readiness.summary,
    };
  }
  if (library.counts.pending_jobs > 0) {
    return {
      label: "准备中",
      pillClass: "pending",
      summary: `当前库还有 ${library.counts.pending_jobs} 个后台任务未完成。`,
    };
  }
  if (readiness.status === "观察未稳定" || readiness.status === "需要关注" || readiness.status === "配置需关注") {
    return {
      label: "部分受限",
      pillClass: readiness.pillClass,
      summary: readiness.summary,
    };
  }
  return {
    label: "Ready",
    pillClass: "ready",
    summary: "当前库可直接执行搜索和结果复用。",
  };
}

export function currentWorkspaceMeta() {
  if (state.activeWorkspace === "inventory") {
    return {
      title: "库管理",
      summary: "在独立工作区里管理当前库、核对来源状态并浏览详情。",
    };
  }

  if (state.activeWorkspace === "settings") {
    return {
      title: "设置",
      summary: "在同一处调整模型提供方、内容类型、当前库覆盖和诊断信息。",
    };
  }

  return {
    title: "搜索",
    summary: "把建库、导入、搜索、阅读结果和对象复用收束到同一主舞台里完成。",
  };
}

export function sleep(ms) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export function isTerminalJobStatus(status) {
  return ["completed", "failed", "canceled"].includes(status);
}

export function jobPillClass(status) {
  if (status === "completed") {
    return "ready";
  }
  if (status === "failed" || status === "canceled") {
    return "error";
  }
  if (status === "queued" || status === "running") {
    return "pending";
  }
  return "muted";
}

export function canCancelJob(job: JobSnapshot) {
  return job.cancelable && !isTerminalJobStatus(job.status);
}

export function canRetryJob(job: JobSnapshot) {
  return job.retryable && (job.status === "failed" || job.status === "canceled");
}

export function canResumeJob(job: JobSnapshot) {
  return job.retryable && (job.status === "failed" || job.status === "canceled");
}

export function formatJobAttemptLabel(job: JobSnapshot) {
  const parts = [`第 ${job.current_attempt.attempt} 次尝试`];
  if (job.retried_from_job_id) {
    parts.push(`重试自 ${job.retried_from_job_id}`);
  }
  return parts.join(" · ");
}

export function contentTypeResolvedStatusLabel(status: string) {
  if (status === "available") {
    return "已就绪";
  }
  if (status === "not_enabled") {
    return "连接未启用";
  }
  if (status === "runtime_unavailable") {
    return "运行时受限";
  }
  if (status === "not_supported") {
    return "当前不支持";
  }
  return status;
}

export function contentTypeReadinessEntries() {
  const contentTypes = availableContentTypeKeys(
    state.globalContentTypes,
    state.libraryContentTypes,
    state.resolvedContentModels ? { content_types: state.resolvedContentModels.content_types } : null
  );

  return contentTypes.map((contentType) => {
    const binding =
      state.libraryContentTypes.content_types[contentType] ??
      state.globalContentTypes.content_types[contentType] ??
      defaultContentTypeBinding();
    const resolved = state.resolvedContentModels?.content_types?.[contentType];
    const selection = selectionFromBinding(binding);
    const hasOverride = libraryContentTypeHasOverride(contentType);

    if (!binding.enabled) {
      return {
        contentType,
        statusLabel: "已停用",
        pillClass: "muted",
        summary: "当前不参与这个库的后续搜索与入库。",
        context: hasOverride ? "当前库覆盖" : "沿用全局默认",
      };
    }

    if (!binding.model) {
      return {
        contentType,
        statusLabel: "未配置模型",
        pillClass: "error",
        summary: "已经启用，但当前没有绑定模型。",
        context: hasOverride ? "当前库覆盖" : "沿用全局默认",
      };
    }

    if (!resolved) {
      return {
        contentType,
        statusLabel: "等待解析",
        pillClass: "pending",
        summary: `${composeModelReference(selection) || binding.model} 尚未出现在当前 resolved model 摘要里。`,
        context: hasOverride ? "当前库覆盖" : "沿用全局默认",
      };
    }

    if (resolved.status !== "available") {
      return {
        contentType,
        statusLabel: contentTypeResolvedStatusLabel(resolved.status),
        pillClass: providerSelectionPillClass(resolved.status),
        summary: resolved.message,
        context: formatResolvedContentModelContext(resolved),
      };
    }

    return {
      contentType,
      statusLabel: "已就绪",
      pillClass: "ready",
      summary: formatResolvedContentModel(resolved),
      context: `${formatBindingSource(resolved.binding_source)} · 向量类型 ${resolved.vector_type}`,
    };
  });
}

export function libraryOperationalReadiness(library: LibrarySnapshot) {
  const enabledRoots = state.sourceRoots.filter((item) => item.enabled);
  const degradedRoots = state.sourceRoots.filter((item) => item.status === "degraded");
  const nonWatchingRoots = enabledRoots.filter((item) => item.watch_state !== "watching");
  const watchIssues = nonWatchingRoots.length;
  const searchableUnits = library.counts.accepted_items;
  const pendingJobs = library.counts.pending_jobs;
  const contentTypeEntries = contentTypeReadinessEntries();
  const blockedContentTypes = contentTypeEntries.filter(
    (entry) => entry.pillClass === "error" || entry.pillClass === "pending"
  );
  const lastActionSummary =
    state.sourceRoots.map((item) => item.last_action?.summary).find(Boolean) ?? "";

  if (!state.sourceRoots.length) {
    return {
      status: "尚未接入来源根",
      pillClass: "muted",
      summary: "这个库还没有来源根。先接入一个本地目录来源根，再执行 refresh 或 rescan。",
      enabledRoots: 0,
      degradedRoots: 0,
      watchIssues: 0,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (!enabledRoots.length) {
    return {
      status: "来源根已停用",
      pillClass: "muted",
      summary: "当前所有来源根都已停用；恢复至少一个来源根后，这个库才会继续承接 watcher、refresh 与 rescan。",
      enabledRoots: 0,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (pendingJobs > 0 && searchableUnits <= 0) {
    return {
      status: "正在准备中",
      pillClass: "pending",
      summary: "当前已有来源根接入，后台任务正在导入或建索引；任务完成后，这个库就会进入可搜索状态。",
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (degradedRoots.length > 0) {
    const details = [];
    details.push(`${degradedRoots.length} 个来源根处于需关注状态`);
    if (watchIssues > 0) {
      details.push(`${watchIssues} 个启用来源根当前不在监视中`);
    }
    details.push(
      searchableUnits > 0
        ? `当前仍有 ${searchableUnits} 个可搜索对象可继续使用`
        : "建议先检查来源根，再执行一次 refresh 或 rescan"
    );
    return {
      status: "需要关注",
      pillClass: "error",
      summary: `${details.join("，")}。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (watchIssues > 0) {
    return {
      status: "观察未稳定",
      pillClass: "pending",
      summary:
        searchableUnits > 0
          ? `${watchIssues} 个启用来源根当前不在监视中，但这个库仍有 ${searchableUnits} 个可搜索对象。`
          : `${watchIssues} 个启用来源根当前不在监视中；建议先恢复监视或手动执行 refresh / rescan。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (blockedContentTypes.length > 0 && searchableUnits <= 0) {
    return {
      status: "等待配置",
      pillClass: "pending",
      summary: `${blockedContentTypes.length} 个启用内容类型当前还未就绪；先检查当前库覆盖与 resolved model，再继续导入或搜索。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (blockedContentTypes.length > 0) {
    return {
      status: "配置需关注",
      pillClass: "pending",
      summary: `${blockedContentTypes.length} 个启用内容类型当前未完全就绪；已有对象仍可搜索，但后续入库会受当前库覆盖与 resolved model 影响。`,
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  if (searchableUnits <= 0) {
    return {
      status: "等待内容",
      pillClass: "muted",
      summary: "来源根已经接入，但这个库还没有可搜索对象。先导入首批内容，或对现有来源执行 refresh / rescan。",
      enabledRoots: enabledRoots.length,
      degradedRoots: degradedRoots.length,
      watchIssues,
      searchableUnits,
      pendingJobs,
      blockedContentTypes: blockedContentTypes.length,
      lastActionSummary,
    };
  }

  return {
    status: "可搜索",
    pillClass: "ready",
    summary: `当前库已接入 ${enabledRoots.length} 个启用来源根，${searchableUnits} 个对象可以直接参与搜索。`,
    enabledRoots: enabledRoots.length,
    degradedRoots: degradedRoots.length,
    watchIssues,
    searchableUnits,
    pendingJobs,
    blockedContentTypes: blockedContentTypes.length,
    lastActionSummary,
  };
}

export function retiredVectorSpaceDiagnostics() {
  return (state.vectorSpaceDiagnostics?.vector_spaces ?? []).filter(
    (vectorSpace) => Boolean(vectorSpace.cleanup_summary)
  );
}
