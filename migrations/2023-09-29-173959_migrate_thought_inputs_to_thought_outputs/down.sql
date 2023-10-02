-- This file should undo anything in `up.sql`
DELETE FROM thought_outputs WHERE interaction_type = 'inpt';
