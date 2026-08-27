import type { ReactNode } from "react";

interface WidgetFrameProps {
  title: string;
  actions?: ReactNode;
  children: ReactNode;
}

/** Marco común de los widgets: cabecera arrastrable y cuerpo con scroll propio. */
export function WidgetFrame({ title, actions, children }: WidgetFrameProps) {
  return (
    <div className="card" style={{ height: "100%" }}>
      <div className="card__header widget__drag-handle">
        <h3 className="card__title">{title}</h3>
        <div className="row">{actions}</div>
      </div>
      <div className="card__body">{children}</div>
    </div>
  );
}

export function WidgetEmpty({ message }: { message: string }) {
  return (
    <div className="empty" style={{ padding: "24px 8px" }}>
      <span className="small">{message}</span>
    </div>
  );
}
