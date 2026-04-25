import type { LibrarySnapshot } from "../../../types";
import { retiredVectorSpaceDiagnostics } from "../../selectors/runtime";
import { state } from "../../state/store";

export function renderInventoryActionRow(library: LibrarySnapshot | null) {
  const retiredVectorSpaces = retiredVectorSpaceDiagnostics();
  return `
    <div class="inventory-action-stack">
      <div class="inline-actions inventory-action-row">
        <button
          type="button"
          class="ui-button ui-button-secondary"
          data-testid="inventory-action-manage-source-roots"
          data-inventory-source-management-toggle
          aria-expanded="${state.inventorySourceManagementOpen ? "true" : "false"}"
          ${library ? "" : "disabled"}
        >
          ${state.inventorySourceManagementOpen ? "收起来源根" : "管理来源根"}
        </button>
        <button
          type="button"
          class="ui-button ui-button-secondary"
          data-testid="inventory-action-refresh-library"
          data-utilities-action="refresh-library"
          ${library ? "" : "disabled"}
        >
          刷新当前库
        </button>
        <button
          type="button"
          class="ui-button ui-button-secondary"
          data-testid="inventory-action-rescan-library"
          data-utilities-action="rescan-library"
          ${library ? "" : "disabled"}
        >
          重扫当前库
        </button>
        <button
          type="button"
          class="ui-button ui-button-secondary"
          data-testid="inventory-action-library-maintenance"
          data-inventory-library-maintenance-toggle
          aria-expanded="${state.inventoryLibraryMaintenanceOpen ? "true" : "false"}"
          ${library ? "" : "disabled"}
        >
          ${state.inventoryLibraryMaintenanceOpen ? "收起库维护" : "库维护"}
        </button>
      </div>
      ${
        state.inventoryLibraryMaintenanceOpen
          ? `
            <div class="inventory-library-maintenance-panel" data-testid="inventory-library-maintenance-panel">
              <div class="inventory-library-maintenance-head">
                <div>
                  <p class="eyebrow">库维护</p>
                  <h4>低频维护动作</h4>
                </div>
                <p class="helper" data-testid="inventory-library-maintenance-summary">
                  ${retiredVectorSpaces.length ? `退役执行空间 ${retiredVectorSpaces.length}` : "当前没有退役执行空间待清理"}
                </p>
              </div>
              <div class="inline-actions inventory-library-maintenance-actions">
                <button
                  type="button"
                  class="ui-button ui-button-secondary"
                  data-testid="inventory-library-maintenance-rebuild"
                  data-utilities-action="rebuild-library"
                  ${library ? "" : "disabled"}
                >
                  重建当前库
                </button>
                <button
                  type="button"
                  class="ui-button ui-button-secondary"
                  data-testid="inventory-library-maintenance-cleanup"
                  data-utilities-action="cleanup-retired-vector-spaces"
                  ${library && retiredVectorSpaces.length ? "" : "disabled"}
                >
                  清理退役执行空间
                </button>
              </div>
            </div>
          `
          : ""
      }
    </div>
  `;
}
