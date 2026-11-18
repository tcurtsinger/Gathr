-- A turn a restart cut stops, never fails (0051): its message reached the agent, which can carry on. Turns cut
-- before read stopped too, with the same why.
UPDATE turns SET status = 'stopped'
 WHERE status = 'failed' AND why = 'the service stopped during this turn';
