import { escapeHtml } from "../../selectors/common";
import { canCancelJob, canResumeJob, canRetryJob, formatJobAttemptLabel, jobPillClass } from "../../selectors/runtime";
import { state } from "../../state/store";
import { renderEmptyState, renderStatusTag, renderUiButton } from "./primitives";

function renderJobProgress(job) {
  const total = job.progress.total;
  const completed = Math.min(Math.max(job.progress.completed, 0), Math.max(total, 0));
  const indeterminate = total <= 0;
  const percent = indeterminate ? 100 : Math.round((completed / total) * 100);
  const unit = job.progress.unit ? ` ${escapeHtml(job.progress.unit)}` : "";
  const label = indeterminate
    ? `等待开始${unit ? ` ·${unit}` : ""}`
    : `${completed}/${total}${unit}`;

  return `
    <div
      class="job-progress ${indeterminate ? "job-progress-indeterminate" : ""}"
      data-testid="job-progress"
      data-progress-kind="${indeterminate ? "indeterminate" : "determinate"}"
    >
      <div class="job-progress-track" aria-hidden="true">
        <span class="job-progress-fill" style="width: ${percent}%"></span>
      </div>
      <small>${label}</small>
    </div>
  `;
}

export function renderJobs() {
  if (!state.selectedLibraryId) {
    return renderEmptyState("先创建或选择一个库，再查看任务。");
  }

  if (!state.jobs.length) {
    return renderEmptyState("当前库还没有任务。");
  }

  return `
    <ul class="job-list" data-testid="job-list">
      ${state.jobs
        .map(
          (job) => `
            <li class="job-card" data-testid="job-card" data-job-id="${escapeHtml(job.job_id)}" data-job-status="${escapeHtml(job.status)}">
              <div class="job-meta">
                ${renderStatusTag(job.status, jobPillClass(job.status) as any)}
                <span>${escapeHtml(job.job_id)}</span>
              </div>
              <h4>${escapeHtml(job.kind)} · ${escapeHtml(job.phase)}</h4>
              <p>${escapeHtml(job.current_attempt.summary)}</p>
              <p class="helper" data-testid="job-attempt-lineage">${escapeHtml(formatJobAttemptLabel(job))}</p>
              ${renderJobProgress(job)}
              <div class="ui-action-row">
                ${
                  canCancelJob(job)
                    ? `
                      ${renderUiButton("取消任务", {
                        tone: "secondary",
                        testId: "job-cancel-button",
                        attrs: { "data-job-cancel-id": job.job_id },
                      })}
                    `
                    : ""
                }
                ${
                  canResumeJob(job)
                    ? `
                      ${renderUiButton("继续任务", {
                        tone: "secondary",
                        testId: "job-resume-button",
                        attrs: { "data-job-resume-id": job.job_id },
                      })}
                    `
                    : ""
                }
                ${
                  canRetryJob(job)
                    ? `
                      ${renderUiButton("重试任务", {
                        tone: "secondary",
                        testId: "job-retry-button",
                        attrs: { "data-job-retry-id": job.job_id },
                      })}
                    `
                    : ""
                }
              </div>
            </li>
          `
        )
        .join("")}
    </ul>
  `;
}
