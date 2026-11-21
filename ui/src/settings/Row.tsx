import { type ReactNode, useId } from 'react';

/** A Settings page: a narrow centred column, its title with one muted line under it, then its rows. */
export function Page({
  title,
  line,
  action,
  back,
  wide,
  children,
}: {
  title: ReactNode;
  line?: ReactNode;
  /** At the title's right: a project's Delete. */
  action?: ReactNode;
  /** Over the title: the link back to the list a page opened from. */
  back?: ReactNode;
  /** The whole width and height, for Turns. */
  wide?: boolean;
  children?: ReactNode;
}) {
  const id = useId();
  return (
    <section className={wide ? 'settings-wide' : 'settings-col'} aria-labelledby={id}>
      {back}
      <header className="settings-head">
        <div>
          <h3 id={id}>{title}</h3>
          {line && <p>{line}</p>}
        </div>
        {action}
      </header>
      {children}
    </section>
  );
}

/** A small heading over a group of rows (`settings-rows settings-top`), with what belongs at its right (Add). */
export function Group({ title, right }: { title: string; right?: ReactNode }) {
  return (
    <div className="settings-group">
      <h4>{title}</h4>
      {right}
    </div>
  );
}

/**
 * The one row of a setting: its name and one muted line at the left, its control at the right. What follows it
 * (a notice, a sign-in, an open editor) takes the row's whole width under them.
 */
export function Row({
  name,
  line,
  lineId,
  lead,
  children,
  after,
}: {
  /** Plain words, or the label of the control. */
  name: ReactNode;
  line?: ReactNode;
  /** For the control's aria-describedby. */
  lineId?: string;
  /** Before the words: a face or a tile. */
  lead?: ReactNode;
  children?: ReactNode;
  after?: ReactNode;
}) {
  return (
    <div className="setting">
      <div className="setting-words">
        {lead}
        <div>
          <div className="setting-name">{name}</div>
          {line && (
            <div className="setting-line" id={lineId}>
              {line}
            </div>
          )}
        </div>
      </div>
      {children && <div className="setting-control">{children}</div>}
      {after}
    </div>
  );
}
