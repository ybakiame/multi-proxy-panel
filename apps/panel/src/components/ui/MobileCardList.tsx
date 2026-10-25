import type { ReactNode } from "react";
import { Card, CardContent } from "./card";

interface MobileCardListProps {
  children: ReactNode;
}

export function MobileCardList({ children }: MobileCardListProps) {
  return <div className="space-y-3 md:hidden">{children}</div>;
}

interface MobileCardProps {
  title: ReactNode;
  status?: ReactNode;
  fields: Array<{ label: ReactNode; value: ReactNode }>;
  actions?: ReactNode;
}

export function MobileCard({ title, status, fields, actions }: MobileCardProps) {
  return (
    <Card size="sm">
      <CardContent className="space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 font-medium break-all">{title}</div>
          {status}
        </div>
        <dl className="grid grid-cols-2 gap-x-4 gap-y-2 text-xs">
          {fields.map((field, index) => (
            <div key={index} className="min-w-0">
              <dt className="text-muted-foreground">{field.label}</dt>
              <dd className="mt-0.5 break-all">{field.value}</dd>
            </div>
          ))}
        </dl>
        {actions && <div className="flex justify-end border-t pt-2">{actions}</div>}
      </CardContent>
    </Card>
  );
}
