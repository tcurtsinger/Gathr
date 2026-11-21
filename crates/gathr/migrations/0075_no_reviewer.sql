-- The Reviewer role is gone (the owner, 2026-11-21): a team is its lead and members. A reviewer stays on its team
-- as a member. The tables' CHECK still allows 'reviewer'; rebuilding both to drop it isn't worth it.
UPDATE chat_members SET role = 'member' WHERE role = 'reviewer';
UPDATE project_members SET role = 'member' WHERE role = 'reviewer';
