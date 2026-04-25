import { registerWorkspaceDrawerScenarios } from "./workspace-drawer";
import { registerWorkspaceJobScenarios } from "./workspace-jobs";
import { registerWorkspaceMobileDetailScenarios } from "./workspace-mobile-detail";
import { registerWorkspaceRefreshPreservationScenarios } from "./workspace-refresh-preservation";
import { registerWorkspaceShellScenarios } from "./workspace-shell";

export {
  registerWorkspaceDrawerScenarios,
  registerWorkspaceJobScenarios,
  registerWorkspaceMobileDetailScenarios,
  registerWorkspaceRefreshPreservationScenarios,
  registerWorkspaceShellScenarios,
};

export function registerWorkspaceRegressionScenarios() {
  registerWorkspaceShellScenarios();
  registerWorkspaceDrawerScenarios();
  registerWorkspaceJobScenarios();
  registerWorkspaceMobileDetailScenarios();
  registerWorkspaceRefreshPreservationScenarios();
}
