import { Button, Chip } from "@pp/ui";
import type { NodeView } from "@pp/client-core";
import { delayColor, delayText, nodeTypeLabel } from "../../components/proxyFormat";

interface NodeItemProps {
  name: string;
  node: NodeView | undefined;
  selected: boolean;
  selectable: boolean;
  isAuto: boolean;
  busy: boolean;
  testing: boolean;
  onSelect: () => void;
  onTest: () => void;
}

export default function NodeItem({
  name,
  node,
  selected,
  selectable,
  isAuto,
  busy,
  testing,
  onSelect,
  onTest,
}: NodeItemProps) {
  const canClick = selectable && !busy && !testing;

  return (
    <div
      className={[
        "flex w-full min-w-0 items-center gap-2 rounded-xl border p-2 transition-colors",
        selected
          ? "border-primary bg-primary/10"
          : "border-border/60 bg-surface hover:border-primary/40 hover:bg-surface-secondary",
      ].join(" ")}
    >
      <button
        type="button"
        className="flex min-h-12 min-w-0 flex-1 flex-col justify-center gap-1 rounded-lg px-2 text-left outline-none focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-default"
        disabled={!canClick}
        onClick={onSelect}
        aria-pressed={selected}
        aria-label={`选择节点 ${name}`}
      >
        <div className="flex w-full min-w-0 items-center gap-1.5">
          <span className="min-w-0 flex-1 truncate text-sm font-medium" title={name}>
            {name}
          </span>
          {selected && (
            <Chip size="sm" variant="soft" color="accent">
              选中
            </Chip>
          )}
          {isAuto && selected && (
            <Chip size="sm" variant="soft" color="default">
              自动
            </Chip>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          {node && (
            <Chip size="sm" variant="soft" color="default">
              {nodeTypeLabel(node.node_type)}
            </Chip>
          )}
          {node?.udp && (
            <Chip size="sm" variant="soft" color="success">
              UDP
            </Chip>
          )}
        </div>
      </button>

      <Button
        size="sm"
        variant="ghost"
        isPending={testing}
        isDisabled={testing}
        onPress={() => {
          onTest();
        }}
        aria-label={`测速 ${name}`}
        className="shrink-0"
      >
        <Chip size="sm" variant="soft" color={delayColor(node?.delay_ms)}>
          {testing ? "测速中" : delayText(node?.delay_ms)}
        </Chip>
      </Button>
    </div>
  );
}
