import { useEffect, useState } from 'react';
import { api, type Failure } from './api';
import { Confirm } from './components/Confirm';
import { ConfirmRemove } from './components/ConfirmRemove';
import { useCard } from './components/Corner';
import { type Deletable, type DeleteCounts, deleteBody, leftFailure } from './deleting';

interface Props {
  kind: Deletable;
  id: string;
  name: string;
  opener?: HTMLElement | null;
  onCancel: () => void;
  /** Done; what it couldn't remove yet, as a failure to show, if anything. */
  onDeleted: (left: Failure | null) => void;
}

/**
 * "Delete <name>?" for a project or an agent (0069): the body is what the service counts when it opens, so
 * it is true. Delete waits for the counts; counts that fail to load are told, and Delete stays off.
 */
export function DeleteConfirm({ kind, id, name, opener, onCancel, onDeleted }: Props) {
  const [counts, failure] = useCounts(kind, id);
  return (
    <Confirm
      question={`Delete ${name}?`}
      body={counts ? deleteBody(kind, counts) : undefined}
      verb="Delete"
      danger
      waiting={!counts}
      loadFailure={failure}
      opener={opener}
      onCancel={onCancel}
      onConfirm={() => api.delete(kind, id).then((done) => onDeleted(leftFailure(done.left)))}
    />
  );
}

/** The service's count of what a delete takes, or why it couldn't count. */
function useCounts(kind: Deletable, id: string) {
  const [counts, setCounts] = useState<DeleteCounts | null>(null);
  const [failure, setFailure] = useState<Failure | null>(null);
  useEffect(() => {
    let current = true;
    api.deleteCounts(kind, id).then(
      (c) => current && setCounts(c),
      (f: Failure) => current && setFailure(f),
    );
    return () => {
      current = false;
    };
  }, [kind, id]);
  return [counts, failure] as const;
}

/** The same question in place of Delete, at a Settings page's title (the owner's pick 5A, 2026-11-24). */
export function DeleteInline({ kind, id, name, onCancel, onDeleted }: Omit<Props, 'opener'>) {
  const [counts, failure] = useCounts(kind, id);
  const [deleting, setDeleting] = useState<Failure | null>(null);
  useCard(failure ?? deleting);
  return (
    <ConfirmRemove
      name={name}
      verb="Delete"
      body={counts ? deleteBody(kind, counts) : undefined}
      waiting={!counts}
      onCancel={onCancel}
      onRemove={() => api.delete(kind, id).then((done) => onDeleted(leftFailure(done.left)), setDeleting)}
    />
  );
}
