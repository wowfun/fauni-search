import { selectedInventorySource } from "../selectors/inventory";
import {
  EDITABLE_TARGET_SELECTOR,
  root,
  setLastRenderedDetailPanelKey,
  state,
  type FocusedEditableState,
} from "./store";

export function selectedVisualUnitDetailSignature(): string | null {
  if (!state.selectedVisualUnit) {
    return null;
  }

  const visualUnit = state.selectedVisualUnit.visual_unit;
  return JSON.stringify({
    library_id: state.selectedVisualUnitLibraryId || state.selectedLibraryId || null,
    visual_unit_id: visualUnit.visual_unit_id,
    source_id: visualUnit.source_id,
    source_path: visualUnit.source_path,
    source_type: visualUnit.source_type,
    kind: visualUnit.kind,
    locator: visualUnit.locator,
    preview_url: state.selectedVisualUnit.preview?.url ?? null,
    neighbor_context: state.selectedVisualUnit.neighbor_context ?? null,
  });
}

export function currentDetailPanelRenderKey(): string | null {
  const detailSignature = selectedVisualUnitDetailSignature();
  if (!detailSignature) {
    return null;
  }

  return JSON.stringify({
    detailSignature,
    searchDetailSheetOpen: state.searchDetailSheetOpen,
  });
}

export function searchDetailSheetIsOpen() {
  return Boolean(state.selectedVisualUnit && state.searchDetailSheetOpen);
}

export function inventoryDetailSheetIsOpen() {
  return Boolean(selectedInventorySource() && state.inventoryDetailSheetOpen);
}

export function captureFocusedEditableState(): FocusedEditableState | null {
  const activeElement = document.activeElement;
  if (
    !(activeElement instanceof HTMLElement) ||
    !root.contains(activeElement) ||
    !activeElement.matches(EDITABLE_TARGET_SELECTOR) ||
    !activeElement.id
  ) {
    return null;
  }

  const snapshot = {
    id: activeElement.id,
    value: null,
    selectionStart: null,
    selectionEnd: null,
  };

  if (
    activeElement instanceof HTMLInputElement ||
    activeElement instanceof HTMLTextAreaElement ||
    activeElement instanceof HTMLSelectElement
  ) {
    snapshot.value = activeElement.value;
  }

  if (
    (activeElement instanceof HTMLInputElement && activeElement.type !== "number") ||
    activeElement instanceof HTMLTextAreaElement
  ) {
    snapshot.selectionStart = activeElement.selectionStart;
    snapshot.selectionEnd = activeElement.selectionEnd;
  }

  return snapshot;
}

export function hasFocusedEditableControl() {
  return captureFocusedEditableState() !== null;
}

export function restoreFocusedEditableState(snapshot: FocusedEditableState | null): void {
  if (!snapshot?.id) {
    return;
  }

  const nextElement = document.getElementById(snapshot.id);
  if (
    !(nextElement instanceof HTMLElement) ||
    !nextElement.matches(EDITABLE_TARGET_SELECTOR) ||
    nextElement.hasAttribute("disabled")
  ) {
    return;
  }

  nextElement.focus({ preventScroll: true });

  if (
    snapshot.value !== null &&
    ((nextElement instanceof HTMLInputElement && nextElement.type !== "file") ||
      nextElement instanceof HTMLTextAreaElement ||
      nextElement instanceof HTMLSelectElement)
  ) {
    nextElement.value = snapshot.value;
  }

  if (
    snapshot.selectionStart !== null &&
    snapshot.selectionEnd !== null &&
    ((nextElement instanceof HTMLInputElement && nextElement.type !== "number") ||
      nextElement instanceof HTMLTextAreaElement)
  ) {
    nextElement.setSelectionRange(snapshot.selectionStart, snapshot.selectionEnd);
  }
}
