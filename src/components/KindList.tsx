import type { AnyKind } from "../lib/types";
import { SUPPORTED_KINDS, CLUSTER_SCOPED_KINDS } from "../lib/types";

interface KindListProps {
  selectedKinds: AnyKind[];
  onToggleKind: (kind: AnyKind) => void;
}

export function KindList({ selectedKinds, onToggleKind }: KindListProps) {
  const allKinds = [...SUPPORTED_KINDS, ...CLUSTER_SCOPED_KINDS];
  const allSelected = selectedKinds.length === allKinds.length;

  const handleToggleAll = () => {
    if (allSelected) {
      allKinds.forEach((kind) => {
        if (selectedKinds.includes(kind)) {
          onToggleKind(kind);
        }
      });
    } else {
      allKinds.forEach((kind) => {
        if (!selectedKinds.includes(kind)) {
          onToggleKind(kind);
        }
      });
    }
  };

  return (
    <div className="kind-list">
      <div className="kind-list-header">
        <h3 className="kind-list-title">Resource Kinds</h3>
        <button
          className="select-all-btn"
          onClick={handleToggleAll}
          type="button"
        >
          {allSelected ? "Deselect All" : "Select All"}
        </button>
      </div>
      <ul className="kind-items">
        {SUPPORTED_KINDS.map((kind) => (
          <li
            key={kind}
            className={`kind-item ${selectedKinds.includes(kind) ? "selected" : ""}`}
          >
            <label className="kind-checkbox-label">
              <input
                type="checkbox"
                checked={selectedKinds.includes(kind)}
                onChange={() => onToggleKind(kind)}
                className="kind-checkbox"
              />
              {kind}
            </label>
          </li>
        ))}
        {CLUSTER_SCOPED_KINDS.length > 0 && (
          <>
            <li className="kind-divider" />
            {CLUSTER_SCOPED_KINDS.map((kind) => (
              <li
                key={kind}
                className={`kind-item ${selectedKinds.includes(kind) ? "selected" : ""}`}
              >
                <label className="kind-checkbox-label">
                  <input
                    type="checkbox"
                    checked={selectedKinds.includes(kind)}
                    onChange={() => onToggleKind(kind)}
                    className="kind-checkbox"
                  />
                  {kind}
                </label>
              </li>
            ))}
          </>
        )}
      </ul>
    </div>
  );
}