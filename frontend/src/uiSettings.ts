const libraryColumnsKey = "anicargo.library.columns";
const defaultLibraryColumns = 4;
const minLibraryColumns = 2;
const maxLibraryColumns = 8;

function clampLibraryColumns(value: number): number {
  if (!Number.isFinite(value)) return defaultLibraryColumns;
  return Math.min(maxLibraryColumns, Math.max(minLibraryColumns, Math.floor(value)));
}

export function loadLibraryColumns(): number {
  const raw = window.localStorage.getItem(libraryColumnsKey);
  if (!raw) return defaultLibraryColumns;
  const parsed = Number(raw);
  return clampLibraryColumns(parsed);
}

export function saveLibraryColumns(value: number): number {
  const normalized = clampLibraryColumns(value);
  window.localStorage.setItem(libraryColumnsKey, normalized.toString());
  return normalized;
}

export const libraryColumnsRange = {
  min: minLibraryColumns,
  max: maxLibraryColumns
};
