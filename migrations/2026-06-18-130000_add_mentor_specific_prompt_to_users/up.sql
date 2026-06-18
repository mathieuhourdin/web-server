ALTER TABLE users
ADD COLUMN mentor_specific_prompt TEXT;

UPDATE users
SET mentor_specific_prompt = ''
WHERE principal_type = 'SERVICE'
   OR id IN (
       SELECT user_id
       FROM user_roles
       WHERE role = 'MENTOR'
   );
