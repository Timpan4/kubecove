import type { AnyKind } from "../lib/types";
import { SUPPORTED_KINDS, CLUSTER_SCOPED_KINDS } from "../lib/types";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

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
    <div className="flex flex-col">
      <div className="mb-3 flex items-center justify-between">
        <h3 className="m-0 text-xs font-semibold uppercase text-muted-foreground">
          Resource Kinds
        </h3>
        <Button
          onClick={handleToggleAll}
          type="button"
          variant="outline"
          size="sm"
          className="h-7 px-2 text-[0.625rem]"
        >
          {allSelected ? "Deselect All" : "Select All"}
        </Button>
      </div>
      <ul className="m-0 list-none p-0">
        {SUPPORTED_KINDS.map((kind) => (
          <li
            key={kind}
            className={cn(
              "cursor-pointer rounded-md p-2 text-sm transition-colors hover:bg-accent",
              selectedKinds.includes(kind) && "bg-accent",
            )}
          >
            <label className="flex cursor-pointer items-center gap-2">
              <input
                type="checkbox"
                checked={selectedKinds.includes(kind)}
                onChange={() => onToggleKind(kind)}
                className="accent-primary"
              />
              {kind}
            </label>
          </li>
        ))}
        {CLUSTER_SCOPED_KINDS.length > 0 && (
          <>
            <li className="my-2 border-t" />
            {CLUSTER_SCOPED_KINDS.map((kind) => (
              <li
                key={kind}
                className={cn(
                  "cursor-pointer rounded-md p-2 text-sm transition-colors hover:bg-accent",
                  selectedKinds.includes(kind) && "bg-accent",
                )}
              >
                <label className="flex cursor-pointer items-center gap-2">
                  <input
                    type="checkbox"
                    checked={selectedKinds.includes(kind)}
                    onChange={() => onToggleKind(kind)}
                    className="accent-primary"
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