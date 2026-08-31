import { Button, Card } from "@heroui/react";

export interface TemplateCardProps {
  template: { id: string; name: string; desc: string };
  applied: boolean;
  onApply: (id: string) => void;
  onRevert: (id: string) => void;
  busy: boolean;
}

export function TemplateCard({ template, applied, onApply, onRevert, busy }: TemplateCardProps) {
  return (
    <Card className="flex flex-col">
      <Card.Header>
        <Card.Title>{template.name}</Card.Title>
        <Card.Description>{template.desc}</Card.Description>
      </Card.Header>
      <Card.Content className="flex-1" />
      <Card.Footer>
        {applied ? (
          <Button variant="secondary" fullWidth isDisabled={busy} onPress={() => onRevert(template.id)}>
            撤销
          </Button>
        ) : (
          <Button variant="primary" fullWidth isDisabled={busy} onPress={() => onApply(template.id)}>
            应用
          </Button>
        )}
      </Card.Footer>
    </Card>
  );
}
